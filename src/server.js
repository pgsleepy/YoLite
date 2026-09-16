import { createReadStream, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { homedir, tmpdir } from "node:os";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import YTMusic from "ytmusic-api";
import youtubedl from "youtube-dl-exec";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, "..");
const publicDir = join(rootDir, "public");
const configPath = process.env.YOLITE_CONFIG || join(homedir(), ".config", "yolite", "config.json");
const port = Number(process.env.PORT || 4173);

let cachedMusic;
let cachedSessionKey = null;
let cachedInit;
const streamCache = new Map();
const pendingStreamResolves = new Map();
const streamCacheTtlMs = 45 * 60 * 1000;

const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".ico": "image/x-icon"
};

function readConfig() {
  try {
    return JSON.parse(readFileSync(configPath, "utf8"));
  } catch {
    return {};
  }
}

function writeConfig(config) {
  mkdirSync(dirname(configPath), { recursive: true });
  writeFileSync(configPath, JSON.stringify(config, null, 2));
}

const maxCookieHeaderBytes = 8 * 1024;

function isYoutubeCookieDomain(domain) {
  const normalized = String(domain).trim().replace(/^\./, "").toLowerCase();
  return normalized === "youtube.com" || normalized.endsWith(".youtube.com");
}

function isSupportedCookieDomain(domain) {
  const normalized = String(domain).trim().replace(/^\./, "").toLowerCase();
  return isYoutubeCookieDomain(normalized) || normalized === "google.com" || normalized.endsWith(".google.com");
}

function isLikelyYoutubeCookieName(name) {
  return [
    "APISID",
    "CONSENT",
    "DEVICE_INFO",
    "GPS",
    "HSID",
    "LOGIN_INFO",
    "PREF",
    "SAPISID",
    "SID",
    "SIDCC",
    "SOCS",
    "SSID",
    "VISITOR_INFO1_LIVE",
    "VISITOR_PRIVACY_METADATA",
    "YSC"
  ].includes(name)
    || name.startsWith("__Secure-1P")
    || name.startsWith("__Secure-3P")
    || name.startsWith("__Secure-Y")
    || name.startsWith("ST-");
}

function cookieName(pair) {
  return pair.split("=", 1)[0];
}

function joinCookiePairs(pairs) {
  const seen = new Set();
  const ordered = [];
  for (let index = pairs.length - 1; index >= 0; index -= 1) {
    const pair = pairs[index];
    const name = cookieName(pair);
    if (!name || seen.has(name)) continue;
    seen.add(name);
    ordered.push(pair);
  }
  return ordered.reverse().join("; ");
}

function compactCookieHeader(pairs) {
  const joined = joinCookiePairs(pairs);
  if (Buffer.byteLength(joined) <= maxCookieHeaderBytes) return joined;
  return joinCookiePairs(pairs.filter(pair => isLikelyYoutubeCookieName(cookieName(pair))));
}

function normalizeCookies(input = "") {
  const raw = String(input).trim();
  if (!raw) return "";

  const headerPairs = raw
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => /^cookie:/i.test(line))
    .flatMap(line => line.replace(/^cookie:\s*/i, "").split(";"))
    .map(part => part.trim())
    .filter(part => part && part.includes("="));

  if (headerPairs.length) return compactCookieHeader(headerPairs);

  const netscapePairs = raw
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => line && !line.startsWith("#"))
    .map(line => {
      const parts = line.split("\t");
      if (parts.length < 7) return null;
      if (!isSupportedCookieDomain(parts[0])) return null;
      return `${parts[5]}=${parts[6]}`;
    })
    .filter(Boolean);

  if (netscapePairs.length) return compactCookieHeader(netscapePairs);

  const pairs = raw
    .replace(/^cookie:\s*/i, "")
    .replace(/[\r\n]+/g, "; ")
    .split(";")
    .map(part => part.trim())
    .filter(part => part && part.includes("=") && !part.includes(":"));
  return compactCookieHeader(pairs);
}

const browserCookieSources = new Set(["firefox", "chrome", "chromium", "brave", "edge", "opera", "vivaldi"]);
const cookieImportProbeUrl = "https://music.youtube.com/watch?v=jNQXAC9IVRw";
const ytdlpAudioFormat = "bestaudio[ext=m4a]/bestaudio[protocol^=http]/bestaudio/best[protocol^=http]/best";
const musicOrigin = "https://music.youtube.com";

function importCookiesFromBrowser(browser) {
  if (!browserCookieSources.has(browser)) {
    return Promise.reject(new Error("Choose a supported browser cookie source"));
  }

  const cookiePath = join(tmpdir(), `yolite-cookies-${process.pid}-${browser}.txt`);
  const args = [
    "--cookies-from-browser",
    browser,
    "--cookies",
    cookiePath,
    "--skip-download",
    "--simulate",
    "--no-warnings",
    "--quiet",
    "--no-playlist",
    cookieImportProbeUrl
  ];

  return new Promise((resolvePromise, reject) => {
    const child = spawn(youtubedl.constants.YOUTUBE_DL_PATH, args, {
      stdio: ["ignore", "ignore", "pipe"]
    });
    let stderr = "";

    child.stderr.on("data", chunk => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", code => {
      try {
        const cookies = normalizeCookies(existsSync(cookiePath) ? readFileSync(cookiePath, "utf8") : "");
        if (cookies) {
          resolvePromise(cookies);
          return;
        }

        if (code !== 0) {
          reject(new Error(stderr.trim() || `Could not import cookies from ${browser}`));
          return;
        }

        reject(new Error(`No YouTube cookies found in ${browser}`));
      } catch (error) {
        reject(error);
      } finally {
        rmSync(cookiePath, { force: true });
      }
    });
  });
}

function json(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body)
  });
  res.end(body);
}

function badRequest(res, message) {
  json(res, 400, { error: message });
}

async function readBody(req) {
  let body = "";
  for await (const chunk of req) {
    body += chunk;
    if (body.length > 1024 * 1024) throw new Error("Request body too large");
  }
  return body ? JSON.parse(body) : {};
}

async function getMusic() {
  const config = readConfig();
  const cookie = config.cookies || "";
  const userId = normalizeUserId(config.userId || config.user_id || "");
  const sessionKey = JSON.stringify({ cookie, userId });
  if (cachedMusic && cachedSessionKey === sessionKey) return cachedMusic;
  if (cachedInit && cachedSessionKey === sessionKey) return cachedInit;

  cachedSessionKey = sessionKey;
  cachedMusic = new YTMusic();
  cachedInit = cachedMusic.initialize(cookie ? { cookies: cookie } : undefined).then(() => {
    if (userId && cachedMusic.client?.interceptors?.request) {
      cachedMusic.client.interceptors.request.use(req => {
        if (req.data?.context?.user) req.data.context.user.onBehalfOfUser = userId;
        return req;
      });
    }
    return cachedMusic;
  });
  return cachedInit;
}

function cookieValue(cookies, name) {
  return cookies
    .split(";")
    .map(part => part.trim().split(/=(.*)/s))
    .find(([cookieName]) => cookieName === name)?.[1] || "";
}

function youtubeAuthHeader(cookies) {
  const sapisid = cookieValue(cookies, "SAPISID")
    || cookieValue(cookies, "__Secure-3PAPISID")
    || cookieValue(cookies, "__Secure-1PAPISID");
  if (!sapisid) return "";
  const timestamp = Math.floor(Date.now() / 1000);
  const hash = createHash("sha1").update(`${timestamp} ${sapisid} ${musicOrigin}`).digest("hex");
  return `SAPISIDHASH ${timestamp}_${hash}`;
}

function extractJsonObject(source, start) {
  let index = start;
  while (/\s/.test(source[index] || "")) index += 1;
  if (source[index] !== "{") return null;
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let cursor = index; cursor < source.length; cursor += 1) {
    const ch = source[cursor];
    if (inString) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === "\"") inString = false;
      continue;
    }
    if (ch === "\"") inString = true;
    else if (ch === "{") depth += 1;
    else if (ch === "}" && --depth === 0) return [source.slice(index, cursor + 1), cursor + 1];
  }
  return null;
}

function extractYtConfig(html) {
  const config = {};
  let offset = 0;
  while (true) {
    const start = html.indexOf("ytcfg.set(", offset);
    if (start < 0) break;
    const extracted = extractJsonObject(html, start + "ytcfg.set(".length);
    if (!extracted) break;
    const [raw, nextOffset] = extracted;
    try {
      Object.assign(config, JSON.parse(raw));
    } catch {
      // YouTube sometimes emits JS object syntax in adjacent config blocks.
    }
    offset = nextOffset;
  }
  return config;
}

function valuesForKey(data, key, out = []) {
  if (Array.isArray(data)) {
    for (const item of data) valuesForKey(item, key, out);
  } else if (data && typeof data === "object") {
    if (Object.hasOwn(data, key)) out.push(data[key]);
    for (const value of Object.values(data)) valuesForKey(value, key, out);
  }
  return out;
}

function nestedText(data) {
  return valuesForKey(data, "text").find(value => typeof value === "string") || "";
}

function carouselTitle(carousel) {
  return nestedText(carousel.header || carousel);
}

function bestRecursiveThumb(data) {
  return valuesForKey(data, "url")
    .filter(value => typeof value === "string" && /^https?:\/\//.test(value))
    .at(-1) || "";
}

function parseHomeTrackCard(item) {
  const endpoint = valuesForKey(item, "watchEndpoint")
    .find(value => value?.videoId);
  if (!endpoint?.videoId) return null;
  const title = nestedText(item.title);
  if (!title) return null;
  const subtitleRuns = valuesForKey(item.subtitle || {}, "runs").flat();
  const artistRun = subtitleRuns.find(run => valuesForKey(run, "pageType").includes("MUSIC_PAGE_TYPE_ARTIST"))
    || subtitleRuns.find(run => {
      const text = run?.text || "";
      return text && text !== "•" && !/^\d+:\d+$/.test(text) && !["Song", "Video", "Album", "Playlist"].includes(text);
    });
  return {
    id: endpoint.videoId,
    type: valuesForKey(item, "musicVideoType").includes("MUSIC_VIDEO_TYPE_ATV") ? "SONG" : "VIDEO",
    title,
    artist: artistRun?.text || "YouTube Music",
    artistId: valuesForKey(artistRun || {}, "browseId")[0] || null,
    album: "",
    duration: 0,
    thumbnail: bestRecursiveThumb(item),
    url: `https://music.youtube.com/watch?v=${endpoint.videoId}`,
    playlistId: endpoint.playlistId || "",
    playlistParams: endpoint.params || ""
  };
}

async function directYoutubeMusicRequest(endpoint, requestBody) {
  const config = readConfig();
  const cookies = config.cookies || "";
  const userId = normalizeUserId(config.userId || config.user_id || "");
  const authUser = Number(config.auth_user || 0);
  const userAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_4) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.129 Safari/537.36";
  const html = await fetch(`${musicOrigin}/`, {
    headers: { cookie: cookies, "user-agent": userAgent, "accept-language": "en-US,en;q=0.5" }
  }).then(response => response.text());
  const ytConfig = extractYtConfig(html);
  if (!ytConfig.INNERTUBE_API_KEY) return [];
  const body = {
    context: {
      capabilities: {},
      client: {
        clientName: ytConfig.INNERTUBE_CLIENT_NAME,
        clientVersion: ytConfig.INNERTUBE_CLIENT_VERSION,
        gl: ytConfig.GL || "US",
        hl: ytConfig.HL || "en"
      },
      request: { internalExperimentFlags: [], sessionIndex: {} },
      user: { enableSafetyMode: false }
    },
    ...requestBody
  };
  if (userId) body.context.user.onBehalfOfUser = userId;
  const response = await fetch(`${musicOrigin}/youtubei/${ytConfig.INNERTUBE_API_VERSION}/${endpoint}?alt=json&key=${ytConfig.INNERTUBE_API_KEY}&prettyPrint=false`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      origin: musicOrigin,
      referer: `${musicOrigin}/`,
      "x-origin": musicOrigin,
      "x-goog-visitor-id": ytConfig.VISITOR_DATA || "",
      "x-youtube-client-name": ytConfig.INNERTUBE_CLIENT_NAME,
      "x-youtube-client-version": ytConfig.INNERTUBE_CLIENT_VERSION,
      "x-youtube-utc-offset": "0",
      "x-youtube-time-zone": "UTC",
      "x-goog-authuser": String(authUser),
      authorization: youtubeAuthHeader(cookies),
      cookie: cookies
    },
    body: JSON.stringify(body)
  });
  if (!response.ok) throw new Error(`YouTube Music ${endpoint} failed with ${response.status}`);
  return response.json();
}

async function directHomeSections() {
  const payload = await directYoutubeMusicRequest("browse", { browseId: "FEmusic_home" });
  return valuesForKey(payload, "musicCarouselShelfRenderer")
    .map(section => {
      const seen = new Set();
      const tracks = valuesForKey(section, "musicTwoRowItemRenderer")
        .map(parseHomeTrackCard)
        .filter(track => track?.id && !seen.has(track.id) && seen.add(track.id))
        .slice(0, 24);
      return { title: carouselTitle(section), tracks, layout: "grid" };
    })
    .filter(section => section.title && section.tracks.length)
    .slice(0, 8);
}

function parseDurationText(value = "") {
  const parts = String(value).split(":").map(part => Number(part));
  if (!parts.length || parts.some(part => !Number.isInteger(part) || part < 0)) return 0;
  return parts.reduce((total, part) => total * 60 + part, 0);
}

function directListTrack(item) {
  const endpoint = valuesForKey(item, "watchEndpoint").find(value => value?.videoId) || {};
  const id = endpoint.videoId || valuesForKey(item, "videoId").find(value => validVideoId(value)) || "";
  if (!id) return null;
  const runs = valuesForKey(item, "runs").flat();
  const titleRun = runs.find(run => valuesForKey(run, "musicVideoType").some(type => String(type).startsWith("MUSIC_VIDEO_TYPE_")))
    || runs[0];
  const artistRun = runs.find(run => valuesForKey(run, "pageType").includes("MUSIC_PAGE_TYPE_ARTIST"))
    || runs.find(run => {
      const text = run?.text || "";
      return text && text !== "•" && !/^\d+:\d+$/.test(text) && !["Song", "Video", "Album", "Playlist"].includes(text);
    });
  const durationText = runs.map(run => run?.text || "").find(text => /^\d+:\d+$/.test(text)) || "";
  const title = titleRun?.text || nestedText(item);
  if (!title) return null;
  return {
    id,
    type: valuesForKey(item, "musicVideoType").includes("MUSIC_VIDEO_TYPE_ATV") ? "SONG" : "VIDEO",
    title,
    artist: artistRun?.text || "YouTube Music",
    artistId: valuesForKey(artistRun || {}, "browseId")[0] || null,
    album: "",
    duration: parseDurationText(durationText),
    thumbnail: bestRecursiveThumb(item),
    url: `https://music.youtube.com/watch?v=${id}`,
    playlistId: endpoint.playlistId || "",
    playlistParams: endpoint.params || ""
  };
}

function parsePlaylistCard(item) {
  const title = nestedText(item.title || item);
  let id = valuesForKey(item, "playlistId").find(value => typeof value === "string")
    || valuesForKey(item, "browseId").find(value => typeof value === "string" && /^VL?[a-zA-Z0-9_-]{2,}$/.test(value))
    || "";
  id = id.replace(/^VL/, "");
  if (!title || !/^[a-zA-Z0-9_-]{2,}$/.test(id)) return null;
  return {
    id,
    title,
    thumbnail: bestRecursiveThumb(item),
    url: `https://music.youtube.com/playlist?list=${id}`
  };
}

async function directLibraryPlaylists() {
  const payloads = await Promise.all([
    directYoutubeMusicRequest("browse", { browseId: "FEmusic_library_landing" }).catch(() => null),
    directYoutubeMusicRequest("browse", { browseId: "FEmusic_liked_playlists" }).catch(() => null)
  ]);
  const seen = new Set();
  return payloads
    .filter(Boolean)
    .flatMap(payload => [
      ...valuesForKey(payload, "musicTwoRowItemRenderer"),
      ...valuesForKey(payload, "musicResponsiveListItemRenderer")
    ])
    .map(parsePlaylistCard)
    .filter(playlist => playlist?.id && !seen.has(playlist.id) && seen.add(playlist.id))
    .slice(0, 250);
}

async function directPlaylistTracks(playlistId, limit = 5000) {
  playlistId = normalizePlaylistId(playlistId);
  let payload = await directYoutubeMusicRequest("browse", { browseId: `VL${playlistId}` });
  const tracks = [];
  const seen = new Set();
  while (payload && tracks.length < limit) {
    for (const item of valuesForKey(payload, "musicResponsiveListItemRenderer")) {
      const track = directListTrack(item);
      if (track?.id && !seen.has(track.id)) {
        seen.add(track.id);
        tracks.push(track);
      }
      if (tracks.length >= limit) break;
    }
    const token = valuesForKey(payload, "continuation")
      .find(value => typeof value === "string" && value.length > 8);
    payload = token && tracks.length < limit
      ? await directYoutubeMusicRequest("browse", { continuation: token }).catch(() => null)
      : null;
  }
  return tracks;
}

function directPanelTrack(item) {
  const endpoint = valuesForKey(item, "watchEndpoint").find(value => value?.videoId) || {};
  const id = endpoint.videoId || item.videoId || "";
  if (!id) return null;
  const title = nestedText(item.title) || "Untitled";
  const artist = nestedText(item.longBylineText || item.shortBylineText) || "YouTube Music";
  return {
    id,
    type: "VIDEO",
    title,
    artist,
    artistId: valuesForKey(item.longBylineText || item.shortBylineText || {}, "browseId")[0] || null,
    album: "",
    duration: parseDurationText(nestedText(item.lengthText)),
    thumbnail: bestRecursiveThumb(item.thumbnail || item),
    url: `https://music.youtube.com/watch?v=${id}`,
    playlistId: endpoint.playlistId || "",
    playlistParams: endpoint.params || ""
  };
}

function trackIdentity(track) {
  return `${String(track?.title || "").trim().toLowerCase()}::${String(track?.artist || "").trim().toLowerCase()}`;
}

async function directMixTracks(videoId, playlistId = "", playlistParams = "") {
  const body = { videoId };
  if (playlistId) body.playlistId = playlistId;
  if (playlistParams) body.params = playlistParams;
  const payload = await directYoutubeMusicRequest("next", body);
  const seen = new Set();
  const seenIdentity = new Set();
  const tracks = [
    ...valuesForKey(payload, "playlistPanelVideoRenderer").map(directPanelTrack),
    ...valuesForKey(payload, "musicResponsiveListItemRenderer").map(directListTrack)
  ]
    .filter(track => {
      const identity = trackIdentity(track);
      if (!track?.id || seen.has(track.id) || (identity !== "::" && seenIdentity.has(identity))) return false;
      seen.add(track.id);
      if (identity !== "::") seenIdentity.add(identity);
      return true;
    })
    .slice(0, 80);
  const selected = tracks.findIndex(track => track.id === videoId);
  if (selected > 0) tracks.unshift(...tracks.splice(selected, 1));
  return tracks;
}

async function createYouTubePlaylist(title) {
  const cleanTitle = String(title || "").trim();
  if (cleanTitle.length < 1 || cleanTitle.length > 80) throw new Error("Playlist name must be 1-80 characters");
  const payload = await directYoutubeMusicRequest("playlist/create", {
    title: cleanTitle,
    privacyStatus: "PRIVATE"
  });
  const id = payload.playlistId || valuesForKey(payload, "playlistId").find(value => typeof value === "string") || "";
  if (!id) throw new Error("YouTube Music did not return a playlist id");
  return {
    id,
    title: cleanTitle,
    thumbnail: "",
    url: `https://music.youtube.com/playlist?list=${id}`
  };
}

async function addTrackToYouTubePlaylist(playlistId, videoId) {
  await directYoutubeMusicRequest("browse/edit_playlist", {
    playlistId,
    actions: [{
      action: "ACTION_ADD_VIDEO",
      addedVideoId: videoId
    }]
  });
}

async function setTrackLiked(videoId, liked) {
  await directYoutubeMusicRequest(liked ? "like/like" : "like/removelike", {
    target: { videoId }
  });
}

function normalizeUserId(userId = "") {
  return String(userId).trim().replace(/^\/+|\/+$/g, "");
}

function normalizePlaylistId(playlistId = "") {
  const id = String(playlistId).trim().replace(/^VL/, "");
  if (/^[a-zA-Z0-9_-]{2,}$/.test(id)) return id;
  throw new Error("Invalid playlist id");
}

function validVideoId(videoId = "") {
  return /^[a-zA-Z0-9_-]{6,}$/.test(String(videoId));
}

function validPlaylistParams(params = "") {
  return /^[a-zA-Z0-9_%-]*$/.test(String(params));
}

function validateUserId(userId = "") {
  const normalized = normalizeUserId(userId);
  if (!normalized) return normalized;
  if (/^\d{20,22}$/.test(normalized)) return normalized;
  throw new Error("YouTube Music rejected this User ID format. For multiple YouTube accounts under one Gmail, switch to the music account, open https://www.youtube.com/account_advanced to confirm the account, then use the numeric Brand Account ID from the /b/<id>/ URL at https://myaccount.google.com/brandaccounts.");
}

function extractUserIdsFromAccountPage(raw = "") {
  const normalized = String(raw)
    .replaceAll("\\/", "/")
    .replaceAll("\\u002F", "/")
    .replaceAll("\\u002f", "/")
    .replaceAll("%2F", "/")
    .replaceAll("%2f", "/");
  const ids = new Set();
  for (const marker of ["/b/", "/channel/"]) {
    let offset = 0;
    while (offset < normalized.length) {
      const index = normalized.indexOf(marker, offset);
      if (index < 0) break;
      const start = index + marker.length;
      const id = normalized.slice(start).match(/^[a-zA-Z0-9_-]+/)?.[0] || "";
      if (/^\d{20,22}$/.test(id)) ids.add(id);
      offset = start;
    }
  }
  return [...ids].sort();
}

async function detectUserIdsFromAccountPage(cookies = "") {
  if (!cookies.trim()) return [];
  try {
    const response = await fetch("https://www.youtube.com/account_advanced", {
      headers: {
        cookie: cookies,
        "user-agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
      }
    });
    return extractUserIdsFromAccountPage(await response.text());
  } catch {
    return [];
  }
}

async function resolveUserId(cookies, requestedUserId = "", existingUserId = "") {
  const userId = requestedUserId.trim()
    ? validateUserId(requestedUserId)
    : validateUserId(existingUserId);
  if (userId) return userId;
  return (await detectUserIdsFromAccountPage(cookies))[0] || "";
}

function normalizeChannelId(channelId = "") {
  const value = String(channelId).trim().replace(/^\/+|\/+$/g, "");
  return value.includes("/channel/")
    ? value.split("/channel/").pop().replace(/^\/+|\/+$/g, "")
    : value.replace(/^MPLA/, "");
}

function validateChannelId(channelId = "") {
  const normalized = normalizeChannelId(channelId);
  if (!normalized) return normalized;
  if (/^UC[a-zA-Z0-9_-]{18,}$/.test(normalized)) return normalized;
  throw new Error("Use the Channel ID from https://www.youtube.com/account_advanced. It usually starts with UC. The shorter User ID is not accepted for public playlist lookup.");
}

function bestThumb(thumbnails = []) {
  return thumbnails.reduce((best, item) => {
    if (!best) return item;
    return item.width > best.width ? item : best;
  }, null)?.url || "";
}

function normalizeTrack(item) {
  return {
    id: item.videoId,
    type: item.type,
    title: item.name,
    artist: item.artist?.name || "Unknown artist",
    artistId: item.artist?.artistId || null,
    album: item.album?.name || "",
    duration: item.duration || 0,
    thumbnail: bestThumb(item.thumbnails),
    url: `https://music.youtube.com/watch?v=${item.videoId}`
  };
}

function normalizeLibraryTrack(item) {
  return {
    id: item.videoId,
    type: item.type || "VIDEO",
    title: item.name || item.title || "Untitled",
    artist: item.artist?.name || item.artists?.name || "Unknown artist",
    artistId: item.artist?.artistId || item.artists?.artistId || null,
    album: item.album?.name || "",
    duration: item.duration || 0,
    thumbnail: bestThumb(item.thumbnails),
    url: `https://music.youtube.com/watch?v=${item.videoId}`
  };
}

function homeContentToTrack(item) {
  if (!item?.videoId) return null;
  return normalizeLibraryTrack(item);
}

async function handleSearch(req, res, url) {
  const query = (url.searchParams.get("q") || "").trim();
  if (query.length < 2) return json(res, 200, { results: [] });

  const music = await getMusic();
  const [songs, videos] = await Promise.all([
    music.searchSongs(query).catch(() => []),
    music.searchVideos(query).catch(() => [])
  ]);

  const seen = new Set();
  const results = [...songs, ...videos]
    .filter(item => item.videoId && !seen.has(item.videoId) && seen.add(item.videoId))
    .slice(0, 36)
    .map(normalizeTrack);

  json(res, 200, { results });
}

async function getLikedTracks(music) {
  const errors = [];
  for (const playlistId of ["LM", "VLLM"]) {
    try {
      return (await music.getPlaylistVideos(playlistId))
        .filter(item => item.videoId)
        .slice(0, 50)
        .map(normalizeLibraryTrack);
    } catch (error) {
      errors.push(`${playlistId}: ${error.message}`);
    }
  }
  try {
    const tracks = await directPlaylistTracks("LM", 50);
    if (tracks.length) return tracks;
  } catch (error) {
    errors.push(`direct LM: ${error.message}`);
  }
  throw new Error(errors.join("; "));
}

async function handleLibrary(req, res) {
  if (req.method !== "GET") return res.writeHead(405).end();

  if (!readConfig().cookies) {
    return json(res, 200, { needsLogin: true, sections: [] });
  }

  const sections = [];
  const music = await getMusic().catch(() => null);

  if (music) {
    try {
      const likedTracks = await getLikedTracks(music);
      if (likedTracks.length) sections.push({ title: "Liked Music", tracks: likedTracks });
    } catch {
      // Some accounts do not expose Liked Music through playlist browse.
    }
  } else {
    const likedTracks = await directPlaylistTracks("LM", 50).catch(() => []);
    if (likedTracks.length) sections.push({ title: "Liked Music", tracks: likedTracks });
  }

  const playlists = await directLibraryPlaylists().catch(() => []);
  json(res, 200, { needsLogin: false, sections, playlists });
}

async function handlePlaylist(req, res, playlistId) {
  if (req.method === "POST" && !playlistId) {
    try {
      const body = await readBody(req);
      return json(res, 200, { playlist: await createYouTubePlaylist(body.title) });
    } catch (error) {
      return json(res, 502, { error: error.message || "Could not create playlist" });
    }
  }
  if (req.method !== "GET") return res.writeHead(405).end();
  try {
    playlistId = normalizePlaylistId(playlistId);

    const music = await getMusic().catch(() => null);
    let tracks = [];
    if (music) {
      try {
        tracks = (await music.getPlaylistVideos(playlistId))
          .filter(item => item.videoId)
          .slice(0, 5000)
          .map(normalizeLibraryTrack);
      } catch {
        tracks = await directPlaylistTracks(playlistId, 5000);
      }
    } else {
      tracks = await directPlaylistTracks(playlistId, 5000);
    }

    json(res, 200, { tracks });
  } catch (error) {
    json(res, 502, { error: error.message || "Could not load playlist" });
  }
}

async function handlePlaylistTracks(req, res, playlistId) {
  if (req.method !== "POST") return res.writeHead(405).end();
  try {
    playlistId = normalizePlaylistId(playlistId);
    const body = await readBody(req);
    const videoId = String(body.videoId || "");
    if (!validVideoId(videoId)) return badRequest(res, "Invalid video id");
    await addTrackToYouTubePlaylist(playlistId, videoId);
    json(res, 200, { saved: true });
  } catch (error) {
    json(res, 502, { error: error.message || "Could not save to playlist" });
  }
}

async function handleLike(req, res) {
  if (req.method !== "POST") return res.writeHead(405).end();
  try {
    const body = await readBody(req);
    const videoId = String(body.videoId || "");
    if (!validVideoId(videoId)) return badRequest(res, "Invalid video id");
    await setTrackLiked(videoId, Boolean(body.liked));
    json(res, 200, { liked: Boolean(body.liked) });
  } catch (error) {
    json(res, 502, { error: error.message || "Could not update like" });
  }
}

async function handleMix(req, res, url, videoId) {
  if (req.method !== "GET") return res.writeHead(405).end();
  if (!validVideoId(videoId)) return badRequest(res, "Invalid video id");
  let playlistId = "";
  try {
    playlistId = url.searchParams.get("playlistId")
      ? normalizePlaylistId(url.searchParams.get("playlistId"))
      : "";
  } catch (error) {
    return badRequest(res, error.message);
  }
  const playlistParams = url.searchParams.get("params") || "";
  if (!validPlaylistParams(playlistParams)) return badRequest(res, "Invalid playlist params");

  try {
    const tracks = await directMixTracks(videoId, playlistId, playlistParams);
    json(res, 200, { tracks });
  } catch (error) {
    json(res, 502, { error: error.message || "Could not load mix" });
  }
}

async function handleHome(req, res) {
  if (req.method !== "GET") return res.writeHead(405).end();

  const sections = [];

  sections.push(...await directHomeSections().catch(() => []));

  if (!sections.length) {
    const music = await getMusic();
    try {
      const home = await music.getHomeSections();
      for (const section of home) {
        const seen = new Set();
        const tracks = (section.contents || [])
          .map(homeContentToTrack)
          .filter(track => track?.id && !seen.has(track.id) && seen.add(track.id))
          .slice(0, 24);
        if (section.title && tracks.length) sections.push({ title: section.title, tracks, layout: "grid" });
        if (sections.length >= 8) break;
      }
    } catch {
      // The current ytmusic-api npm build can fail parsing home sections.
    }
  }

  json(res, 200, { needsLogin: false, sections });
}

function ytdlpFlags(cookiePath) {
  const headers = [
    "referer:https://music.youtube.com",
    "user-agent:Mozilla/5.0"
  ];

  const flags = {
    format: ytdlpAudioFormat,
    noPlaylist: true,
    noWarnings: true,
    quiet: true,
    addHeader: headers
  };
  if (cookiePath) flags.cookies = cookiePath;
  return flags;
}

function ytdlpArgs(cookiePath, extra = []) {
  const args = [
    "--format",
    ytdlpAudioFormat,
    "--no-playlist",
    "--no-warnings",
    "--quiet",
    "--add-header",
    "referer:https://music.youtube.com",
    "--add-header",
    "user-agent:Mozilla/5.0",
    ...extra
  ];
  if (cookiePath) args.push("--cookies", cookiePath);
  return args;
}

function writeYtdlpCookieFile(cookies) {
  if (!cookies) return null;
  const cookiePath = join(tmpdir(), `yolite-ytdlp-cookies-${process.pid}-${Date.now()}-${Math.random().toString(36).slice(2)}.txt`);
  const expires = Math.floor(Date.now() / 1000) + 30 * 24 * 60 * 60;
  const lines = ["# Netscape HTTP Cookie File"];
  for (const pair of cookies.split(";").map(part => part.trim()).filter(Boolean)) {
    const index = pair.indexOf("=");
    if (index <= 0) continue;
    const name = pair.slice(0, index).trim();
    const value = pair.slice(index + 1).trim();
    if (!name || /[\r\n\t]/.test(value)) continue;
    lines.push(`.youtube.com\tTRUE\t/\tTRUE\t${expires}\t${name}\t${value}`);
  }
  writeFileSync(cookiePath, lines.join("\n"));
  return cookiePath;
}

async function resolveStreamWithCookies(videoId, cookies) {
  const cookiePath = writeYtdlpCookieFile(cookies);
  const url = `https://www.youtube.com/watch?v=${videoId}`;
  try {
    const output = await youtubedl(url, {
      ...ytdlpFlags(cookiePath),
      getUrl: true
    });
    return String(output).trim().split(/\r?\n/).at(-1);
  } finally {
    if (cookiePath) rmSync(cookiePath, { force: true });
  }
}

async function resolveStream(videoId) {
  const cached = streamCache.get(videoId);
  if (cached && cached.expiresAt > Date.now()) return cached.url;
  if (pendingStreamResolves.has(videoId)) return pendingStreamResolves.get(videoId);

  const cookies = readConfig().cookies || "";
  const promise = (async () => {
    try {
      return await resolveStreamWithCookies(videoId, "");
    } catch (anonymousError) {
      if (!cookies) throw anonymousError;
      try {
        return await resolveStreamWithCookies(videoId, cookies);
      } catch (cookieError) {
        throw new Error(`yt-dlp failed without cookies: ${anonymousError.message}; with cookies: ${cookieError.message}`);
      }
    }
  })().then(url => {
    if (url) streamCache.set(videoId, { url, expiresAt: Date.now() + streamCacheTtlMs });
    return url;
  }).finally(() => {
    pendingStreamResolves.delete(videoId);
  });
  pendingStreamResolves.set(videoId, promise);
  return promise;
}

async function handleResolve(res, videoId) {
  if (!/^[a-zA-Z0-9_-]{6,}$/.test(videoId)) return badRequest(res, "Invalid video id");
  const streamUrl = await resolveStream(videoId);
  if (!streamUrl) return json(res, 502, { error: "No playable stream found" });
  json(res, 200, { streamUrl, fallbackUrl: `/stream/${videoId}` });
}

async function handlePrefetch(res, videoId) {
  if (!/^[a-zA-Z0-9_-]{6,}$/.test(videoId)) return badRequest(res, "Invalid video id");
  resolveStream(videoId).catch(() => {});
  json(res, 202, { queued: true });
}

async function handleProxy(req, res, videoId) {
  if (!/^[a-zA-Z0-9_-]{6,}$/.test(videoId)) return badRequest(res, "Invalid video id");

  const child = spawn(
    youtubedl.constants.YOUTUBE_DL_PATH,
    [
      ...ytdlpArgs("", ["--output", "-"]),
      "--",
      `https://www.youtube.com/watch?v=${videoId}`
    ],
    { stdio: ["ignore", "pipe", "pipe"] }
  );

  res.writeHead(200, {
    "content-type": "audio/mp4",
    "cache-control": "no-store"
  });

  child.stdout.pipe(res);
  child.stderr.on("data", () => {});
  child.on("error", error => {
    if (!res.headersSent) json(res, 502, { error: error.message });
  });
  req.on("close", () => child.kill("SIGTERM"));
}

async function handleSession(req, res) {
  if (req.method === "GET") {
    const config = readConfig();
    const cookies = config.cookies || "";
    return json(res, 200, {
      loggedIn: Boolean(cookies),
      cookieBytes: Buffer.byteLength(cookies),
      configPath,
      userId: normalizeUserId(config.userId || config.user_id || ""),
      channelId: normalizeChannelId(config.channelId || config.channel_id || "")
    });
  }

  if (req.method === "DELETE") {
    writeConfig({ cookies: "", userId: "", channelId: "" });
    cachedMusic = undefined;
    cachedSessionKey = null;
    cachedInit = undefined;
    return json(res, 200, { loggedIn: false });
  }

  if (req.method === "POST") {
    const body = await readBody(req);
    const existing = readConfig();
    const normalizedCookies = normalizeCookies(body.cookies);
    const cookies = normalizedCookies || existing.cookies || "";
    if (!cookies) return badRequest(res, "Paste a Cookie header or cookies.txt export");
    let userId;
    let channelId;
    try {
      userId = await resolveUserId(cookies, body.userId || body.user_id || "", existing.userId || existing.user_id || "");
      channelId = validateChannelId(body.channelId || body.channel_id || "");
    } catch (error) {
      return badRequest(res, error.message);
    }
    writeConfig({ cookies, userId, channelId });
    cachedMusic = undefined;
    cachedSessionKey = null;
    cachedInit = undefined;
    await getMusic();
    return json(res, 200, {
      loggedIn: true,
      cookieBytes: Buffer.byteLength(cookies),
      userId,
      channelId
    });
  }

  res.writeHead(405).end();
}

async function handleSessionImport(req, res) {
  if (req.method !== "POST") return res.writeHead(405).end();

  const body = await readBody(req);
  const browser = String(body.browser || "").trim();
  const existing = readConfig();
  const cookies = await importCookiesFromBrowser(browser);
  let userId;
  let channelId;
  try {
    userId = await resolveUserId(cookies, body.userId || body.user_id || "", existing.userId || existing.user_id || "");
    channelId = validateChannelId(body.channelId || body.channel_id || "");
  } catch (error) {
    return badRequest(res, error.message);
  }
  writeConfig({ cookies, userId, channelId });
  cachedMusic = undefined;
  cachedSessionKey = null;
  cachedInit = undefined;
  await getMusic();
  json(res, 200, {
    loggedIn: true,
    cookieBytes: Buffer.byteLength(cookies),
    userId,
    channelId
  });
}

function serveStatic(req, res, pathname) {
  const requested = pathname === "/" ? "/index.html" : pathname;
  const filePath = resolve(publicDir, `.${decodeURIComponent(requested)}`);
  if (!filePath.startsWith(publicDir) || !existsSync(filePath) || !statSync(filePath).isFile()) {
    res.writeHead(404).end("Not found");
    return;
  }

  res.writeHead(200, {
    "content-type": types[extname(filePath)] || "application/octet-stream",
    "cache-control": "no-store"
  });
  createReadStream(filePath).pipe(res);
}

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://${req.headers.host}`);

    if (url.pathname === "/api/search" && req.method === "GET") return handleSearch(req, res, url);
    if (url.pathname === "/api/home") return handleHome(req, res);
    if (url.pathname === "/api/library") return handleLibrary(req, res);
    if (url.pathname === "/api/like") return handleLike(req, res);
    if (url.pathname === "/api/playlist" && req.method === "POST") {
      return handlePlaylist(req, res, "");
    }
    if (url.pathname.startsWith("/api/mix/") && req.method === "GET") {
      return handleMix(req, res, url, decodeURIComponent(url.pathname.split("/").pop()));
    }
    if (url.pathname.startsWith("/api/playlist/") && url.pathname.endsWith("/tracks")) {
      return handlePlaylistTracks(req, res, decodeURIComponent(url.pathname.split("/").at(-2)));
    }
    if (url.pathname.startsWith("/api/playlist/") && req.method === "GET") {
      return handlePlaylist(req, res, decodeURIComponent(url.pathname.split("/").pop()));
    }
    if (url.pathname === "/api/session/import") return handleSessionImport(req, res);
    if (url.pathname === "/api/session") return handleSession(req, res);
    if (url.pathname.startsWith("/api/resolve/") && req.method === "GET") {
      return handleResolve(res, url.pathname.split("/").pop());
    }
    if (url.pathname.startsWith("/api/prefetch/") && ["GET", "POST"].includes(req.method)) {
      return handlePrefetch(res, url.pathname.split("/").pop());
    }
    if (url.pathname.startsWith("/stream/") && req.method === "GET") {
      return handleProxy(req, res, url.pathname.split("/").pop());
    }

    serveStatic(req, res, url.pathname);
  } catch (error) {
    json(res, 500, { error: error.message || "Unexpected server error" });
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Yolite running at http://127.0.0.1:${port}`);
});

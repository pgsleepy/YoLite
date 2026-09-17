import { createVisualizer } from "./visualizer.js";

const audio = document.querySelector("#audio");
const visualizer = document.querySelector("#visualizer");
const visualizerRenderer = createVisualizer(visualizer);
const splash = document.querySelector("#splash");
const shell = document.querySelector("#shell");
const searchForm = document.querySelector("#searchForm");
const searchInput = document.querySelector("#searchInput");
const searchChips = document.querySelector("#searchChips");
const homeEl = document.querySelector("#home");
const discoverEl = document.querySelector("#discover");
const resultsEl = document.querySelector("#results");
const libraryEl = document.querySelector("#library");
const historyEl = document.querySelector("#history");
const queueEl = document.querySelector("#queue");
const sidebarPlaylists = document.querySelector("#sidebarPlaylists");
const statusEl = document.querySelector("#status");
const sessionState = document.querySelector("#sessionState");
const browserSource = document.querySelector("#browserSource");
const openYoutubeMusic = document.querySelector("#openYoutubeMusic");
const importBrowserCookies = document.querySelector("#importBrowserCookies");
const appLoginActions = document.querySelector("#appLoginActions");
const appLoginHelp = document.querySelector("#appLoginHelp");
const openAppLogin = document.querySelector("#openAppLogin");
const useAppLogin = document.querySelector("#useAppLogin");
const cookieInput = document.querySelector("#cookieInput");
const userIdInput = document.querySelector("#userIdInput");
const channelIdInput = document.querySelector("#channelIdInput");
const saveCookies = document.querySelector("#saveCookies");
const clearCookies = document.querySelector("#clearCookies");
const refreshLibrary = document.querySelector("#refreshLibrary");
const refreshDiscover = document.querySelector("#refreshDiscover");
const clearQueue = document.querySelector("#clearQueue");
const playBtn = document.querySelector("#playBtn");
const playShape = document.querySelector("#playShape");
const prevBtn = document.querySelector("#prevBtn");
const nextBtn = document.querySelector("#nextBtn");
const loopBtn = document.querySelector("#loopBtn");
const queueToggle = document.querySelector("#queueToggle");
const queueDrawer = document.querySelector("#queueDrawer");
const seek = document.querySelector("#seek");
const elapsed = document.querySelector("#elapsed");
const duration = document.querySelector("#duration");
const volume = document.querySelector("#volume");
const settingsVolume = document.querySelector("#settingsVolume");
const settingsCrossfade = document.querySelector("#settingsCrossfade");
const crossfadeMode = document.querySelector("#crossfadeMode");
const crossfadeValue = document.querySelector("#crossfadeValue");
const settingsState = document.querySelector("#settingsState");
const resetEqualizer = document.querySelector("#resetEqualizer");
const eqControls = [...document.querySelectorAll(".eq-control")];
const presetButtons = [...document.querySelectorAll("[data-preset]")];
const mixLength = document.querySelector("#mixLength");
const mixLengthValue = document.querySelector("#mixLengthValue");
const discoverFromHistory = document.querySelector("#discoverFromHistory");
const discordPresence = document.querySelector("#discordPresence");
const discordPresenceRow = document.querySelector("#discordPresenceRow");
const volumeNormalization = document.querySelector("#volumeNormalization");
const visualizerEnabled = document.querySelector("#visualizerEnabled");
const visualizerFps = document.querySelector("#visualizerFps");
const visualizerFpsValue = document.querySelector("#visualizerFpsValue");
const visualizerBassColor = document.querySelector("#visualizerBassColor");
const visualizerMidsColor = document.querySelector("#visualizerMidsColor");
const visualizerTrebleColor = document.querySelector("#visualizerTrebleColor");
const resetVisualizer = document.querySelector("#resetVisualizer");
const prefetchCount = document.querySelector("#prefetchCount");
const prefetchCountValue = document.querySelector("#prefetchCountValue");
const cacheLimit = document.querySelector("#cacheLimit");
const cacheLimitValue = document.querySelector("#cacheLimitValue");
const playlistWarnLimit = document.querySelector("#playlistWarnLimit");
const playlistWarnLimitValue = document.querySelector("#playlistWarnLimitValue");
const likeBtn = document.querySelector("#likeBtn");
const saveTrackBtn = document.querySelector("#saveTrackBtn");
const newPlaylist = document.querySelector("#newPlaylist");
const playlistDialog = document.querySelector("#playlistDialog");
const closePlaylistDialog = document.querySelector("#closePlaylistDialog");
const playlistSelect = document.querySelector("#playlistSelect");
const saveToPlaylist = document.querySelector("#saveToPlaylist");
const newPlaylistName = document.querySelector("#newPlaylistName");
const createPlaylist = document.querySelector("#createPlaylist");
const nowArt = document.querySelector("#nowArt");
const nowTitle = document.querySelector("#nowTitle");
const nowArtist = document.querySelector("#nowArtist");
const trackMenu = document.querySelector("#trackMenu");
const profileButton = document.querySelector("#profileButton");
const profilePicture = document.querySelector("#profilePicture");
const profileFallback = document.querySelector("#profileFallback");
const profileMenu = document.querySelector("#profileMenu");
const profileName = document.querySelector("#profileName");
const profileUserId = document.querySelector("#profileUserId");
const profileLogin = document.querySelector("#profileLogin");
const profileSettings = document.querySelector("#profileSettings");
const miniPlayerButton = document.querySelector("#miniPlayerButton");
const remoteButton = document.querySelector("#remoteButton");
const remoteDialog = document.querySelector("#remoteDialog");
const remoteQr = document.querySelector("#remoteQr");
const remoteUrl = document.querySelector("#remoteUrl");
const reloadPlugins = document.querySelector("#reloadPlugins");
const pluginList = document.querySelector("#pluginList");
const resetHotkeys = document.querySelector("#resetHotkeys");
const hotkeyState = document.querySelector("#hotkeyState");
const hotkeyInputs = [...document.querySelectorAll("[data-hotkey]")];
const automaticUpdateChecks = document.querySelector("#automaticUpdateChecks");
const updateVersion = document.querySelector("#updateVersion");
const updateState = document.querySelector("#updateState");
const updateNotes = document.querySelector("#updateNotes");
const checkForUpdateButton = document.querySelector("#checkForUpdate");
const installUpdateButton = document.querySelector("#installUpdate");
const isTauri = Boolean(window.__TAURI__?.core?.invoke);

let results = [];
let homeSections = [];
let librarySections = [];
let libraryNeedsLogin = false;
let playlists = [];
let activePlaylist = null;
let activePlaylistTracks = [];
let queue = [];
let currentIndex = -1;
let currentTrack = null;
let currentFallbackUrl = "";
let activeSearchScope = "all";
let resolving = false;
let usingFallback = false;
let nativePlaying = false;
let nativePaused = false;
let nativePosition = 0;
let nativeDuration = 0;
let nativeProgressTimer = 0;
let playRequestId = 0;
let likedTracks = new Set();
let crossfadeStarted = false;
let audioContext = null;
let audioSource = null;
let audioPreamp = null;
let audioFilters = [];
let audioCompressor = null;
let audioAnalyser = null;
let audioGraphFailed = false;
let loopMode = "off";
let playlistDialogTrack = null;
let contextTrack = null;
let listenAgainPage = 0;
let miniPlayer = false;
let loginPollTimer = 0;
let pluginCategories = [];
let visualizerFrame = 0;
let visualizerLastFrame = 0;
let visualizerAnalyserData = null;
let nativeVisualizerRequest = false;
let nativeVisualizerUpdatedAt = 0;
let nativeVisualizerLevels = { bass: 0, mids: 0, treble: 0, energy: 0, peak: 0 };
let visualizerTargetFps = 30;

const defaultHotkeys = {
  playPause: "Ctrl+Alt+Space",
  next: "Ctrl+Alt+ArrowRight",
  previous: "Ctrl+Alt+ArrowLeft",
  volumeUp: "Ctrl+Alt+ArrowUp",
  volumeDown: "Ctrl+Alt+ArrowDown"
};

const defaultVisualizerSettings = {
  fps: 30,
  bass: "#d46e42",
  mids: "#994033",
  treble: "#572929",
};

const equalizerBands = [
  { key: "bass", frequency: 90, type: "lowshelf" },
  { key: "lowMid", frequency: 250, type: "peaking" },
  { key: "mid", frequency: 1000, type: "peaking" },
  { key: "highMid", frequency: 4000, type: "peaking" },
  { key: "treble", frequency: 10000, type: "highshelf" }
];

const equalizerPresets = {
  flat: { preamp: 0, bass: 0, lowMid: 0, mid: 0, highMid: 0, treble: 0 },
  bass: { preamp: -2, bass: 6, lowMid: 3, mid: 0, highMid: -1, treble: 1 },
  warm: { preamp: -1, bass: 3, lowMid: 2, mid: 1, highMid: 0, treble: -2 },
  bright: { preamp: -1, bass: -2, lowMid: 0, mid: 1, highMid: 3, treble: 5 },
  vocal: { preamp: -1, bass: -2, lowMid: 1, mid: 4, highMid: 3, treble: 1 }
};

function formatTime(value) {
  if (!Number.isFinite(value) || value <= 0) return "0:00";
  const total = Math.floor(value);
  const minutes = Math.floor(total / 60);
  const seconds = String(total % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function setStatus(message) {
  statusEl.textContent = message;
}

function errorMessage(error, fallback = "Unexpected error") {
  if (error?.message) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}

function renderUpdateStatus(status) {
  updateVersion.textContent = `v${status.currentVersion}`;
  updateNotes.textContent = status.notes || "";
  updateNotes.hidden = !status.notes;
  installUpdateButton.hidden = !status.available || status.managedExternally;
  if (status.managedExternally) {
    updateState.textContent = "Managed by pacman. Update with paru/yay or your preferred AUR helper.";
    checkForUpdateButton.hidden = true;
  } else if (status.available) {
    updateState.textContent = `Yolite ${status.version} is ready to install.`;
  } else {
    updateState.textContent = "Yolite is up to date.";
  }
}

async function checkForUpdates({ quiet = false } = {}) {
  if (!isTauri) {
    updateVersion.textContent = "Desktop only";
    updateState.textContent = "Install a desktop build to receive signed updates from GitHub Releases.";
    checkForUpdateButton.disabled = true;
    automaticUpdateChecks.disabled = true;
    return;
  }

  checkForUpdateButton.disabled = true;
  if (!quiet) updateState.textContent = "Checking GitHub Releases…";
  try {
    const status = await window.__TAURI__.core.invoke("check_for_update");
    renderUpdateStatus(status);
  } catch (error) {
    updateState.textContent = errorMessage(error, "Could not check for updates");
  } finally {
    checkForUpdateButton.disabled = false;
  }
}

async function installAvailableUpdate() {
  installUpdateButton.disabled = true;
  checkForUpdateButton.disabled = true;
  updateState.textContent = "Downloading and verifying update…";
  try {
    await window.__TAURI__.core.invoke("install_update");
    updateState.textContent = "Update installed. Restarting Yolite…";
  } catch (error) {
    updateState.textContent = errorMessage(error, "Could not install update");
    installUpdateButton.disabled = false;
    checkForUpdateButton.disabled = false;
  }
}

function sessionMessage(payload) {
  if (!payload.loggedIn) return "No cookies saved";
  const userSuffix = payload.userId ? ", YouTube account selected" : "";
  const channelSuffix = payload.channelId ? ", channel playlists included" : "";
  if (payload.libraryAuthenticated === true) return `${payload.cookieBytes} bytes saved${userSuffix}${channelSuffix}, library ready`;
  if (payload.libraryAuthenticated === false) {
    return `${payload.cookieBytes} bytes saved${userSuffix}${channelSuffix}, checking library access`;
  }
  return `${payload.cookieBytes} bytes saved${userSuffix}${channelSuffix}`;
}

function applySession(payload) {
  userIdInput.value = payload.userId || "";
  channelIdInput.value = payload.channelId || "";
  sessionState.textContent = sessionMessage(payload);
  profileName.textContent = payload.profileName || (payload.loggedIn ? "YouTube Music account" : "Not signed in");
  profileUserId.textContent = payload.userId ? `User ID ${payload.userId}` : "No YouTube User ID";
  if (payload.profilePicture) {
    profilePicture.src = payload.profilePicture;
    profileFallback.hidden = true;
  } else {
    profilePicture.removeAttribute("src");
    profileFallback.hidden = false;
    profileFallback.textContent = payload.loggedIn ? (payload.profileName || "Y").slice(0, 1).toUpperCase() : "?";
  }
}

function crossfadeSeconds() {
  return Number(settingsCrossfade.value) || 0;
}

function shouldFadeTransition(reason) {
  if (!crossfadeSeconds()) return false;
  return crossfadeMode.value === "always" || reason === "auto";
}

function saveCrossfadeMode(value) {
  crossfadeMode.value = value === "always" ? "always" : "auto";
  localStorage.setItem("yolite:crossfadeMode", crossfadeMode.value);
  settingsState.textContent = "Saved locally";
}

function saveLikedTracks() {
  syncLikeButton();
}

function storedJson(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem(key) || JSON.stringify(fallback));
  } catch {
    return fallback;
  }
}

function visualizerSettings() {
  return {
    fps: Number(visualizerFps.value) || defaultVisualizerSettings.fps,
    bass: visualizerBassColor.value,
    mids: visualizerMidsColor.value,
    treble: visualizerTrebleColor.value,
  };
}

function applyVisualizerSettings(settings, save = true) {
  const allowedFrameRates = [15, 30, 60, 120];
  const fps = allowedFrameRates.includes(Number(settings?.fps))
    ? Number(settings.fps)
    : defaultVisualizerSettings.fps;
  const color = (value, fallback) => /^#[0-9a-f]{6}$/i.test(value) ? value : fallback;
  visualizerFps.value = String(fps);
  visualizerTargetFps = fps;
  visualizerBassColor.value = color(settings?.bass, defaultVisualizerSettings.bass);
  visualizerMidsColor.value = color(settings?.mids, defaultVisualizerSettings.mids);
  visualizerTrebleColor.value = color(settings?.treble, defaultVisualizerSettings.treble);
  visualizerFpsValue.textContent = `${fps} FPS`;
  visualizer.dataset.fps = String(fps);
  visualizerRenderer.setColors(visualizerSettings());
  visualizerLastFrame = 0;
  if (save) {
    localStorage.setItem("yolite:visualizerSettings", JSON.stringify(visualizerSettings()));
    settingsState.textContent = "Saved locally";
  }
}

function currentEqualizer() {
  const values = { ...equalizerPresets.flat, normalization: Boolean(volumeNormalization?.checked) };
  for (const control of eqControls) {
    values[control.dataset.eq] = Number(control.value) || 0;
  }
  return values;
}

function setEqualizer(values) {
  for (const control of eqControls) {
    if (Object.hasOwn(values, control.dataset.eq)) {
      control.value = String(values[control.dataset.eq]);
    }
  }
  updateSettingsLabels();
  localStorage.setItem("yolite:equalizer", JSON.stringify(currentEqualizer()));
  applyEqualizer();
}

function dbText(value) {
  const number = Number(value) || 0;
  return `${number > 0 ? "+" : ""}${number} dB`;
}

function updateSettingsLabels() {
  crossfadeValue.textContent = `${crossfadeSeconds()}s`;
  mixLengthValue.textContent = `${Number(mixLength.value) || 18} tracks`;
  prefetchCountValue.textContent = `${prefetchWindowSize()} tracks`;
  cacheLimitValue.textContent = `${Number(cacheLimit.value) || 0} GB`;
  playlistWarnLimitValue.textContent = `${Number(playlistWarnLimit.value) || 100} songs`;
  for (const control of eqControls) {
    const output = document.querySelector(`#${control.id}Value`);
    if (output) output.textContent = dbText(control.value);
  }
}

function ensureAudioGraph() {
  if (isTauri) return false;
  if (audioGraphFailed) return false;
  if (audioContext) return true;
  try {
    audioContext = new AudioContext();
    audioSource = audioContext.createMediaElementSource(audio);
    audioPreamp = audioContext.createGain();
    audioCompressor = audioContext.createDynamicsCompressor();
    audioAnalyser = audioContext.createAnalyser();
    audioAnalyser.fftSize = 128;
    audioAnalyser.smoothingTimeConstant = 0.82;
    audioFilters = equalizerBands.map(band => {
      const filter = audioContext.createBiquadFilter();
      filter.type = band.type;
      filter.frequency.value = band.frequency;
      if (band.type === "peaking") filter.Q.value = 1;
      return filter;
    });
    let node = audioSource;
    for (const filter of audioFilters) {
      node.connect(filter);
      node = filter;
    }
    node.connect(audioCompressor);
    audioCompressor.connect(audioPreamp);
    audioPreamp.connect(audioAnalyser);
    audioAnalyser.connect(audioContext.destination);
    return true;
  } catch {
    audioGraphFailed = true;
    return false;
  }
}

function applyBrowserEqualizer() {
  if (!ensureAudioGraph()) return;
  const values = currentEqualizer();
  audioPreamp.gain.value = Math.pow(10, values.preamp / 20);
  for (const [index, band] of equalizerBands.entries()) {
    audioFilters[index].gain.value = values[band.key] || 0;
  }
  audioCompressor.threshold.value = values.normalization ? -24 : 0;
  audioCompressor.knee.value = values.normalization ? 24 : 0;
  audioCompressor.ratio.value = values.normalization ? 8 : 1;
  audioCompressor.attack.value = values.normalization ? 0.01 : 0;
  audioCompressor.release.value = values.normalization ? 0.25 : 0.01;
}

function applyEqualizer() {
  const equalizer = currentEqualizer();
  if (isTauri) {
    window.__TAURI__.core.invoke("set_native_equalizer", { equalizer }).catch(error => {
      setStatus(errorMessage(error, "Could not update equalizer"));
    });
  } else {
    applyBrowserEqualizer();
  }
  settingsState.textContent = "Saved locally";
}

function applyVolume(value) {
  const next = Math.max(0, Math.min(1, Number(value) || 0));
  volume.value = String(next);
  settingsVolume.value = String(next);
  localStorage.setItem("yolite:volume", String(next));
  if (isTauri) {
    window.__TAURI__.core.invoke("set_native_volume", { volume: next }).catch(error => {
      setStatus(errorMessage(error, "Could not change volume"));
    });
  }
  audio.volume = next;
  syncRemoteState();
}

function applyCrossfade(value) {
  const next = Math.max(0, Math.min(10, Number(value) || 0));
  settingsCrossfade.value = String(next);
  localStorage.setItem("yolite:crossfade", String(next));
  updateSettingsLabels();
}

function loadHistory() {
  return storedJson("yolite:history", []);
}

function saveHistory(items) {
  localStorage.setItem("yolite:history", JSON.stringify(items.slice(0, 80)));
}

function loadRecentPlays() {
  const plays = storedJson("yolite:recentPlays", []);
  if (plays.length) return plays.slice(0, 50);
  return loadHistory()
    .filter(item => item?.id && item.title)
    .sort((a, b) => (b.lastPlayed || 0) - (a.lastPlayed || 0))
    .slice(0, 50);
}

function saveRecentPlay(track) {
  const plays = loadRecentPlays();
  plays.unshift({ ...track, playedAt: Date.now() });
  localStorage.setItem("yolite:recentPlays", JSON.stringify(plays.slice(0, 50)));
}

function playlistCacheKey(playlistId) {
  return `yolite:playlist:${playlistId}:tracks`;
}

function loadCachedPlaylist(playlistId) {
  return storedJson(playlistCacheKey(playlistId), null);
}

function saveCachedPlaylist(playlistId, tracks) {
  if (!playlistId || !tracks?.length) return;
  localStorage.setItem(playlistCacheKey(playlistId), JSON.stringify({
    savedAt: Date.now(),
    tracks: tracks.slice(0, 5000),
  }));
}

function describePlaylistChanges(previous = [], next = []) {
  const previousIds = previous.map(track => track.id).filter(Boolean);
  const nextIds = next.map(track => track.id).filter(Boolean);
  const previousSet = new Set(previousIds);
  const nextSet = new Set(nextIds);
  const added = nextIds.filter(id => !previousSet.has(id)).length;
  const removed = previousIds.filter(id => !nextSet.has(id)).length;
  const reordered = !added && !removed && previousIds.some((id, index) => nextIds[index] !== id);
  const parts = [];
  if (added) parts.push(`${added} added`);
  if (removed) parts.push(`${removed} removed`);
  if (reordered) parts.push("order changed");
  return parts.join(", ");
}

function recordPlayHistory(track) {
  if (!track?.id) return;
  saveRecentPlay(track);
  const items = loadHistory();
  const existing = items.find(item => item.id === track.id);
  if (existing) {
    existing.count = (existing.count || 0) + 1;
    existing.lastPlayed = Date.now();
    Object.assign(existing, track);
  } else {
    items.unshift({ ...track, count: 1, lastPlayed: Date.now() });
  }
  saveHistory(items.sort((a, b) => (b.lastPlayed || 0) - (a.lastPlayed || 0)));
  renderHistory();
}

async function api(path, options) {
  const invoke = window.__TAURI__?.core?.invoke;
  if (invoke) {
    const method = options?.method || "GET";
    if (path.startsWith("/api/search")) {
      return invoke("search_tracks", { query: new URL(path, location.origin).searchParams.get("q") || "" });
    }
    if (path === "/api/home") return invoke("get_home");
    if (path === "/api/library") return invoke("get_library");
    if (path.startsWith("/api/mix/")) {
      const url = new URL(path, location.origin);
      return invoke("get_mix", {
        videoId: decodeURIComponent(url.pathname.split("/").pop()),
        playlistId: url.searchParams.get("playlistId") || "",
        playlistParams: url.searchParams.get("params") || ""
      });
    }
    if (path.startsWith("/api/prefetch/")) {
      return invoke("prefetch_track", { videoId: decodeURIComponent(path.split("/").pop()) });
    }
    if (path.startsWith("/api/playlist/")) {
      const parts = path.split("/");
      if (parts.at(-1) === "tracks" && method === "POST") {
        const body = JSON.parse(options.body || "{}");
        return invoke("add_track_to_playlist", {
          playlistId: decodeURIComponent(parts.at(-2)),
          videoId: body.videoId || ""
        });
      }
      return invoke("get_playlist", { playlistId: decodeURIComponent(parts.pop()) });
    }
    if (path === "/api/playlist" && method === "POST") {
      const body = JSON.parse(options.body || "{}");
      return invoke("create_playlist", { title: body.title || "" });
    }
    if (path === "/api/like" && method === "POST") {
      const body = JSON.parse(options.body || "{}");
      return invoke("set_track_liked", { videoId: body.videoId || "", liked: Boolean(body.liked) });
    }
    if (path.startsWith("/api/resolve/")) {
      return invoke("resolve_track", { videoId: decodeURIComponent(path.split("/").pop()) });
    }
    if (path === "/api/session" && method === "GET") return invoke("get_session");
    if (path === "/api/session" && method === "DELETE") return invoke("clear_session");
    if (path === "/api/session/import" && method === "POST") {
      const body = JSON.parse(options.body || "{}");
      return invoke("import_browser_session", { browser: body.browser || "", userId: body.userId || "", channelId: body.channelId || "" });
    }
    if (path === "/api/session/app-login/open" && method === "POST") return invoke("open_app_login");
    if (path === "/api/session/app-login/import" && method === "POST") {
      const body = JSON.parse(options.body || "{}");
      return invoke("import_app_login_session", { userId: body.userId || "", channelId: body.channelId || "" });
    }
    if (path === "/api/session" && method === "POST") {
      const body = JSON.parse(options.body || "{}");
      return invoke("save_session", { cookies: body.cookies || "", userId: body.userId || "", channelId: body.channelId || "" });
    }
  }

  const response = await fetch(path, options);
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload.error || response.statusText);
  return payload;
}

function trackRow(track, index, source, collection) {
  const row = document.createElement("button");
  row.className = "track";
  row.type = "button";
  row.dataset.id = track.id;
  row.innerHTML = `
    <span class="track-num">${index + 1}</span>
    <img class="cover" alt="">
    <span class="meta">
      <span class="name"></span>
      <span class="artist"></span>
    </span>
    <span class="album"></span>
    <span class="time">${formatTime(track.duration)}</span>
  `;
  row.querySelector(".name").textContent = track.title;
  row.querySelector(".artist").textContent = track.artist;
  row.querySelector(".album").textContent = track.album;
  const cover = row.querySelector(".cover");
  if (track.thumbnail) cover.src = track.thumbnail;
  else cover.toggleAttribute("data-empty", true);
  cover.addEventListener("error", event => {
    event.currentTarget.removeAttribute("src");
    event.currentTarget.toggleAttribute("data-empty", true);
  });
  row.addEventListener("click", async () => {
    if (source === "queue") {
      currentIndex = index;
      renderQueue();
      playTrack(queue[currentIndex], { reason: "manual" });
      return;
    }
    if (source === "playlist" || source === "history") {
      queue = collection.slice(index);
      currentIndex = 0;
      renderQueue();
      playTrack(queue[currentIndex], { reason: "manual" });
      return;
    }
    try {
      await startYouTubeMix(track);
    } catch (error) {
      setStatus(errorMessage(error, "Could not start mix"));
    }
  });
  row.addEventListener("contextmenu", event => openTrackMenu(event, track));
  return row;
}

function renderList(target, items, source) {
  target.textContent = "";
  if (!items.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = source === "results" ? "Search for music" : "Queue is empty";
    target.append(empty);
    return;
  }
  items.forEach((track, index) => target.append(trackRow(track, index, source, items)));
}

function filterResults(items) {
  const query = searchInput.value.trim().toLowerCase();
  if (activeSearchScope === "all") return items;
  if (activeSearchScope === "song") return items.filter(track => track.kind === "SONG" || track.type === "SONG");
  if (activeSearchScope === "video") return items.filter(track => track.kind === "VIDEO" || track.type === "VIDEO");
  return items.filter(track => String(track[activeSearchScope] || "").toLowerCase().includes(query));
}

function renderResults() {
  renderList(resultsEl, filterResults(results), "results");
}

function renderQueue() {
  renderList(queueEl, queue, "queue");
  queueEl.querySelectorAll(".track").forEach((row, index) => {
    row.classList.toggle("playing", index === currentIndex);
  });
}

function renderHistory() {
  renderList(historyEl, loadRecentPlays(), "history");
  if (!loadRecentPlays().length) {
    historyEl.querySelector(".empty").textContent = "Songs you play will appear here";
  }
}

function syncLikedTracksFromLibrary() {
  const liked = librarySections.find(section => section.title?.toLowerCase() === "liked music");
  if (!liked) return;
  likedTracks = new Set((liked.tracks || []).map(track => track.id).filter(Boolean));
  syncLikeButton();
}

function renderPlaylistSelect() {
  playlistSelect.textContent = "";
  if (!playlists.length) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No playlists loaded";
    playlistSelect.append(option);
    return;
  }
  for (const playlist of playlists) {
    const option = document.createElement("option");
    option.value = playlist.id;
    option.textContent = playlist.title;
    playlistSelect.append(option);
  }
}

function setQueueOpen(open) {
  shell.classList.toggle("queue-open", open);
  queueDrawer.hidden = !open;
  queueToggle.classList.toggle("active", open);
  queueToggle.setAttribute("aria-pressed", String(open));
}

function syncLikeButton() {
  const liked = Boolean(currentTrack?.id && likedTracks.has(currentTrack.id));
  likeBtn.disabled = !currentTrack?.id;
  saveTrackBtn.disabled = !currentTrack?.id;
  likeBtn.classList.toggle("active", liked);
  likeBtn.setAttribute("aria-label", liked ? "Unlike" : "Like");
  likeBtn.setAttribute("aria-pressed", String(liked));
}

function closeTrackMenu() {
  trackMenu.hidden = true;
  contextTrack = null;
}

function openTrackMenu(event, track) {
  event.preventDefault();
  contextTrack = track;
  trackMenu.hidden = false;
  const menuWidth = 220;
  const menuHeight = 132;
  trackMenu.style.left = `${Math.min(event.clientX, window.innerWidth - menuWidth - 12)}px`;
  trackMenu.style.top = `${Math.min(event.clientY, window.innerHeight - menuHeight - 12)}px`;
  trackMenu.querySelector("button")?.focus();
}

function playTrackNext(track) {
  if (!track?.id) return;
  const insertAt = Math.max(0, currentIndex + 1);
  queue.splice(insertAt, 0, track);
  renderQueue();
  prefetchUpcomingTracks();
  setStatus(`${track.title} plays next`);
}

function renderLibrary() {
  libraryEl.textContent = "";
  if (activePlaylist) {
    renderPlaylistDetail();
    return;
  }
  if (playlists.length) {
    const grid = document.createElement("div");
    grid.className = "playlist-grid";
    for (const playlist of playlists) {
      const item = document.createElement("button");
      item.className = "playlist-card";
      item.type = "button";
      item.innerHTML = `<img alt=""><span></span>`;
      const image = item.querySelector("img");
      if (playlist.thumbnail) image.src = playlist.thumbnail;
      else image.toggleAttribute("data-empty", true);
      item.querySelector("span").textContent = playlist.title;
      item.addEventListener("click", () => openPlaylist(playlist));
      grid.append(item);
    }
    libraryEl.append(grid);
  }
  const emptyText = libraryNeedsLogin
    ? "Imported cookies were not accepted by YouTube Music"
    : "No playlists or liked music found";
  renderSections(libraryEl, librarySections, playlists.length ? "" : emptyText);
}

function renderSidebarPlaylists() {
  sidebarPlaylists.textContent = "";
  renderPlaylistSelect();
  const visible = playlists.slice(0, 12);
  if (!visible.length) {
    const empty = document.createElement("div");
    empty.className = "sidebar-empty";
    empty.textContent = "No playlists";
    sidebarPlaylists.append(empty);
    return;
  }
  for (const playlist of visible) {
    const item = document.createElement("button");
    item.className = "playlist-link";
    item.type = "button";
    item.textContent = playlist.title;
    item.classList.toggle("active", playlist.id === activePlaylist?.id);
    item.addEventListener("click", () => {
      showPanel("libraryPanel");
      openPlaylist(playlist);
    });
    sidebarPlaylists.append(item);
  }
}

function playTrackCollection(tracks) {
  if (!tracks.length) return;
  queue = tracks.slice();
  currentIndex = 0;
  renderQueue();
  prefetchUpcomingTracks();
  playTrack(queue[currentIndex], { reason: "manual" });
}

function addTracksToQueue(tracks) {
  if (!tracks.length) return;
  queue.push(...tracks);
  renderQueue();
  prefetchUpcomingTracks();
  setStatus(`Added ${tracks.length} tracks to queue`);
}

async function downloadPlaylistTracks(tracks, button) {
  const limit = Math.max(25, Number(playlistWarnLimit.value) || 100);
  if (tracks.length > limit) {
    const ok = confirm(`${tracks.length} songs may use a lot of storage and bandwidth. Continue?`);
    if (!ok) return;
  }

  button.disabled = true;
  const originalText = button.textContent;
  let done = 0;
  setStatus(`Caching playlist 0/${tracks.length}`);
  try {
    for (const track of tracks) {
      if (track?.id) await api(`/api/prefetch/${encodeURIComponent(track.id)}`);
      done += 1;
      if (done % 5 === 0 || done === tracks.length) setStatus(`Caching playlist ${done}/${tracks.length}`);
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    setStatus(`Playlist cached ${done}/${tracks.length}`);
  } catch (error) {
    setStatus(errorMessage(error, "Could not cache playlist"));
  } finally {
    button.disabled = false;
    button.textContent = originalText;
  }
}

function renderPlaylistDetail() {
  const tracks = activePlaylistTracks;
  const detail = document.createElement("section");
  detail.className = "playlist-detail";

  const back = document.createElement("button");
  back.className = "playlist-back ghost";
  back.type = "button";
  back.textContent = "Back to Library";
  back.addEventListener("click", () => {
    activePlaylist = null;
    activePlaylistTracks = [];
    renderLibrary();
    renderSidebarPlaylists();
  });

  const hero = document.createElement("div");
  hero.className = "playlist-hero";
  const image = document.createElement("img");
  image.alt = "";
  if (activePlaylist.thumbnail) image.src = activePlaylist.thumbnail;
  else image.toggleAttribute("data-empty", true);
  image.addEventListener("error", event => {
    event.currentTarget.removeAttribute("src");
    event.currentTarget.toggleAttribute("data-empty", true);
  }, { once: true });

  const copy = document.createElement("div");
  copy.className = "playlist-copy";
  const label = document.createElement("span");
  label.className = "playlist-label";
  label.textContent = "Playlist";
  const title = document.createElement("h2");
  title.textContent = activePlaylist.title;
  const count = document.createElement("span");
  count.className = "playlist-meta";
  count.textContent = tracks.length === 1 ? "1 track" : `${tracks.length} tracks`;

  const actions = document.createElement("div");
  actions.className = "playlist-actions";
  const play = document.createElement("button");
  play.className = "playlist-action";
  play.type = "button";
  play.textContent = "Play";
  play.disabled = !tracks.length;
  play.addEventListener("click", () => playTrackCollection(tracks));
  const add = document.createElement("button");
  add.className = "playlist-action ghost";
  add.type = "button";
  add.textContent = "Add to Queue";
  add.disabled = !tracks.length;
  add.addEventListener("click", () => addTracksToQueue(tracks));
  const download = document.createElement("button");
  download.className = "playlist-action ghost";
  download.type = "button";
  download.textContent = "Cache";
  download.disabled = !tracks.length;
  download.addEventListener("click", () => downloadPlaylistTracks(tracks, download));
  actions.append(play, add, download);

  copy.append(label, title, count, actions);
  hero.append(image, copy);
  detail.append(back, hero);

  if (tracks.length) {
    const list = document.createElement("div");
    list.className = "track-list";
    tracks.forEach((track, index) => {
      list.append(trackRow(track, index, "playlist", tracks));
    });
    detail.append(list);
  } else {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "No tracks found";
    detail.append(empty);
  }

  libraryEl.append(detail);
}

function renderSections(target, sections, emptyText) {
  if (!sections.length) {
    if (!emptyText) return;
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = emptyText;
    target.append(empty);
    return;
  }

  for (const section of sections) {
    if (section.layout === "grid") {
      renderTrackGridSection(target, section);
      continue;
    }
    const group = document.createElement("section");
    group.className = "library-section";
    const title = document.createElement("h2");
    title.textContent = section.title;
    const list = document.createElement("div");
    list.className = "track-list";
    section.tracks.forEach((track, index) => {
      list.append(trackRow(track, index, "library", section.tracks));
    });
    group.append(title, list);
    target.append(group);
  }
}

function historyItems() {
  return loadHistory()
    .filter(item => item?.id && item.title)
    .sort((a, b) => (b.count || 0) - (a.count || 0) || (b.lastPlayed || 0) - (a.lastPlayed || 0));
}

function recentHistoryItems() {
  return loadRecentPlays();
}

function mixSeeds() {
  if (!discoverFromHistory.checked) return [];
  const byArtist = new Map();
  for (const track of historyItems()) {
    const artist = (track.artist || "").trim();
    if (!artist || artist === "Unknown artist") continue;
    const current = byArtist.get(artist) || { artist, count: 0, track };
    current.count += track.count || 1;
    if ((track.count || 0) > (current.track.count || 0)) current.track = track;
    byArtist.set(artist, current);
  }
  return [...byArtist.values()]
    .sort((a, b) => b.count - a.count)
    .slice(0, 9);
}

function renderTrackShelf(target, titleText, tracks) {
  if (!tracks.length) return;
  const group = document.createElement("section");
  group.className = "library-section";
  const title = document.createElement("h2");
  title.textContent = titleText;
  const list = document.createElement("div");
  list.className = "track-list";
  const collection = tracks.slice(0, titleText === "Recently played" ? 50 : 6);
  collection.forEach((track, index) => {
    list.append(trackRow(track, index, titleText === "Recently played" ? "history" : "home", collection));
  });
  group.append(title, list);
  target.append(group);
}

function setTrackQueue(tracks, status) {
  queue = tracks;
  currentIndex = 0;
  renderQueue();
  setQueueOpen(true);
  prefetchUpcomingTracks();
  playTrack(queue[currentIndex], { reason: "manual" });
  if (status) setStatus(status);
}

function trackIdentity(track) {
  return `${String(track?.title || "").trim().toLowerCase()}::${String(track?.artist || "").trim().toLowerCase()}`;
}

async function fetchTrackMix(track) {
  const query = new URLSearchParams();
  if (track.playlistId) query.set("playlistId", track.playlistId);
  if (track.playlistParams) query.set("params", track.playlistParams);
  const payload = await api(`/api/mix/${encodeURIComponent(track.id)}?${query}`)
    .catch(() => ({ tracks: [] }));
  const seen = new Set();
  const seenIdentity = new Set();
  const tracks = [];
  for (const item of [track, ...(payload.tracks || [])]) {
    const identity = trackIdentity(item);
    if (!item?.id || seen.has(item.id) || (identity !== "::" && seenIdentity.has(identity))) continue;
    seen.add(item.id);
    if (identity !== "::") seenIdentity.add(identity);
    tracks.push(item);
  }
  const limit = Number(mixLength.value) || 18;
  if (tracks.length < 2) {
    const seed = { artist: track.artist || track.title, track };
    for (const search of mixQueries(seed)) {
      const searchPayload = await api(`/api/search?q=${encodeURIComponent(search)}`)
        .catch(() => ({ results: [] }));
      for (const item of searchPayload.results || []) {
        const identity = trackIdentity(item);
        if (!item?.id || seen.has(item.id) || (identity !== "::" && seenIdentity.has(identity))) continue;
        seen.add(item.id);
        if (identity !== "::") seenIdentity.add(identity);
        tracks.push(item);
        if (tracks.length >= limit) break;
      }
      if (tracks.length >= limit) break;
    }
  }
  return tracks;
}

async function startYouTubeMix(track) {
  if (!track?.id) return;
  setStatus(`Starting ${track.title} mix`);
  queue = [track];
  currentIndex = 0;
  renderQueue();
  setQueueOpen(true);
  if (currentTrack?.id !== track.id || (!nativePlaying && audio.paused)) {
    playTrack(track, { reason: "manual" });
  }
  const tracks = await fetchTrackMix(track);
  queue = tracks.length ? tracks : [track];
  currentIndex = Math.max(0, queue.findIndex(item => item.id === currentTrack?.id));
  renderQueue();
  prefetchUpcomingTracks();
  setStatus(queue.length > 1 ? `${track.title} mix` : `Playing ${track.title}`);
}

function setCardImage(image, src) {
  if (src) image.src = src;
  else image.toggleAttribute("data-empty", true);
  image.addEventListener("error", event => {
    event.currentTarget.removeAttribute("src");
    event.currentTarget.toggleAttribute("data-empty", true);
  }, { once: true });
}

function trackCard(track) {
  const item = document.createElement("button");
  item.className = "track-card";
  item.type = "button";
  item.innerHTML = `<img alt=""><span class="track-card-title"></span><span class="track-card-meta"></span>`;
  setCardImage(item.querySelector("img"), track.thumbnail);
  item.querySelector(".track-card-title").textContent = track.title;
  item.querySelector(".track-card-meta").textContent = track.artist || "YouTube Music";
  item.addEventListener("click", async () => {
    try {
      await startYouTubeMix(track);
    } catch (error) {
      setStatus(errorMessage(error, "Could not start mix"));
    }
  });
  item.addEventListener("contextmenu", event => openTrackMenu(event, track));
  return item;
}

function renderTrackGridSection(target, section) {
  const tracks = section.tracks || [];
  if (!tracks.length) return;
  const group = document.createElement("section");
  group.className = "library-section";
  const title = document.createElement("h2");
  title.textContent = section.title;
  const grid = document.createElement("div");
  grid.className = "track-card-grid";
  const collection = tracks.slice(0, 24);
  collection.forEach(track => {
    grid.append(trackCard(track));
  });
  group.append(title, grid);
  target.append(group);
}

function mixQueries(seed) {
  const artist = seed.artist;
  const title = seed.track?.title || "";
  return [
    `${artist}`,
    `${artist} radio`,
    `${title} similar songs`,
    `${artist} genre mix`
  ].filter(query => query.trim().length >= 2);
}

async function startMix(seed) {
  setStatus(`Building ${seed.artist} mix`);
  const limit = Number(mixLength.value) || 18;
  const seen = new Set();
  const tracks = [];
  if (seed.track?.id) {
    tracks.push(seed.track);
    seen.add(seed.track.id);
  }
  for (const query of mixQueries(seed)) {
    const payload = await api(`/api/search?q=${encodeURIComponent(query)}`).catch(() => ({ results: [] }));
    for (const track of payload.results || []) {
      if (!track?.id || seen.has(track.id)) continue;
      seen.add(track.id);
      tracks.push(track);
      if (tracks.length >= limit) break;
    }
    if (tracks.length >= limit) break;
  }
  if (!tracks.length) {
    setStatus("No mix found");
    return;
  }
  queue = tracks;
  currentIndex = 0;
  renderQueue();
  setQueueOpen(true);
  prefetchUpcomingTracks();
  playTrack(queue[currentIndex], { reason: "manual" });
  setStatus(`${seed.artist} mix`);
}

function renderMixShelf(target, titleText, seeds) {
  if (!seeds.length) return;
  const group = document.createElement("section");
  group.className = "library-section";
  const title = document.createElement("h2");
  title.textContent = titleText;
  const grid = document.createElement("div");
  grid.className = "mix-grid";
  for (const seed of seeds) {
    grid.append(mixCard(seed));
  }
  group.append(title, grid);
  target.append(group);
}

function mixCard(seed) {
  const button = document.createElement("button");
  button.className = "mix-card";
  button.type = "button";
  button.innerHTML = `<img alt=""><span class="mix-title"></span><span class="mix-meta"></span>`;
  setCardImage(button.querySelector("img"), seed.track?.thumbnail);
  button.querySelector(".mix-title").textContent = `${seed.artist} mix`;
  button.querySelector(".mix-meta").textContent = `Based on ${seed.track?.title || seed.artist}`;
  button.addEventListener("click", () => startMix(seed));
  if (seed.track) button.addEventListener("contextmenu", event => openTrackMenu(event, seed.track));
  return button;
}

function isListenAgainSection(section) {
  return /listen again|recent|history/i.test(section?.title || "");
}

function listenAgainTracks() {
  const listenSections = homeSections.filter(isListenAgainSection);
  return uniqueTracks([
    ...sectionTracks(listenSections),
    ...recentHistoryItems(),
    ...sectionTracks(homeSections),
  ]).slice(0, 27);
}

function renderListenAgain(target) {
  const tracks = listenAgainTracks();
  if (!tracks.length) return;
  listenAgainPage = Math.min(listenAgainPage, 2);
  const group = document.createElement("section");
  group.className = "library-section listen-again";
  group.innerHTML = `
    <div class="shelf-heading">
      <h2>Listen again</h2>
      <div class="pager-controls">
        <button class="pager-button" type="button" data-page-direction="-1" aria-label="Previous Listen again page">‹</button>
        <span></span>
        <button class="pager-button" type="button" data-page-direction="1" aria-label="Next Listen again page">›</button>
      </div>
    </div>
    <div class="listen-pages"></div>
  `;
  const pages = group.querySelector(".listen-pages");
  for (let pageIndex = 0; pageIndex < 3; pageIndex += 1) {
    const page = document.createElement("div");
    page.className = "listen-page";
    tracks.slice(pageIndex * 9, pageIndex * 9 + 9).forEach(track => page.append(trackCard(track)));
    pages.append(page);
  }
  const update = () => {
    pages.style.transform = `translateX(-${listenAgainPage * 100 / 3}%)`;
    group.querySelector(".pager-controls span").textContent = `${listenAgainPage + 1}/3`;
    group.querySelector('[data-page-direction="-1"]').disabled = listenAgainPage === 0;
    group.querySelector('[data-page-direction="1"]').disabled = listenAgainPage === 2;
  };
  group.querySelectorAll("[data-page-direction]").forEach(button => {
    button.addEventListener("click", () => {
      listenAgainPage = Math.max(0, Math.min(2, listenAgainPage + Number(button.dataset.pageDirection)));
      update();
    });
  });
  update();
  target.append(group);
}

function trackKey(track) {
  const identity = trackIdentity(track);
  return {
    id: `id:${track.id}`,
    identity: identity === "::" ? "" : `identity:${identity}`,
  };
}

function uniqueTracks(tracks, seen = new Set()) {
  const collection = [];
  for (const track of tracks) {
    if (!track?.id || !track.title) continue;
    const key = trackKey(track);
    if (seen.has(key.id) || (key.identity && seen.has(key.identity))) continue;
    seen.add(key.id);
    if (key.identity) seen.add(key.identity);
    collection.push(track);
  }
  return collection;
}

function sectionTracks(sections) {
  return sections.flatMap(section => section.tracks || []);
}

function discoverTracks() {
  const recommendationSections = homeSections.filter(section => !isListenAgainSection(section));
  const recommendationTracks = uniqueTracks(sectionTracks(recommendationSections));
  if (recommendationTracks.length >= 9) return recommendationTracks;
  return uniqueTracks([
    ...recommendationTracks,
    ...sectionTracks(homeSections),
    ...historyItems(),
  ]);
}

function renderDiscoverShelf(target, titleText, items, cardFactory) {
  const collection = items.slice(0, 12);
  if (!collection.length) return;
  const group = document.createElement("section");
  group.className = "discover-shelf";
  const title = document.createElement("h2");
  title.textContent = titleText;
  const grid = document.createElement("div");
  grid.className = "discover-grid";
  collection.forEach(item => grid.append(cardFactory(item)));
  group.append(title, grid);
  target.append(group);
}

const discoveryCategories = [
  "New releases", "Chill", "Focus", "Energy", "Workout", "Party", "Commute", "Sleep",
  "Rock", "Hip-hop", "R&B", "Electronic", "Indie", "Jazz", "Classical", "Metal", "Soul", "Acoustic"
];

function renderDiscoveryCategories(target) {
  const group = document.createElement("section");
  group.className = "discover-shelf";
  const title = document.createElement("h2");
  title.textContent = "Genres and moods";
  const strip = document.createElement("div");
  strip.className = "category-strip";
  [...new Set([...discoveryCategories, ...pluginCategories])].forEach(category => {
    const button = document.createElement("button");
    button.className = "category-button";
    button.type = "button";
    button.textContent = category;
    button.addEventListener("click", async () => {
      searchInput.value = `${category} music`;
      try {
        const payload = await api(`/api/search?q=${encodeURIComponent(searchInput.value)}`);
        results = payload.results || [];
        renderResults();
        showPanel("searchPanel");
      } catch (error) {
        setStatus(errorMessage(error));
      }
    });
    strip.append(button);
  });
  group.append(title, strip);
  target.append(group);
}

function renderDiscover() {
  discoverEl.textContent = "";
  const seeds = mixSeeds();
  const usedTracks = new Set();
  renderDiscoveryCategories(discoverEl);
  renderDiscoverShelf(discoverEl, "Quick picks", uniqueTracks(discoverTracks(), usedTracks), trackCard);
  renderDiscoverShelf(discoverEl, "Artist radios", seeds, mixCard);
  for (const section of homeSections.filter(section => !isListenAgainSection(section))) {
    renderDiscoverShelf(discoverEl, section.title, uniqueTracks(section.tracks || [], usedTracks), trackCard);
  }
  if (discoverEl.children.length) return;
  const empty = document.createElement("div");
  empty.className = "empty";
  empty.textContent = "Play a few tracks to build discovery mixes";
  discoverEl.append(empty);
}

function renderHome() {
  homeEl.textContent = "";
  const lastTrack = loadLastTrack();
  if (lastTrack) {
    const group = document.createElement("section");
    group.className = "library-section";
    const title = document.createElement("h2");
    title.textContent = "Continue where you left off";
    const list = document.createElement("div");
    list.className = "track-list";
    list.append(trackRow(lastTrack, 0, "home", [lastTrack]));
    group.append(title, list);
    homeEl.append(group);
  }
  renderListenAgain(homeEl);
  renderSections(homeEl, homeSections.filter(section => !isListenAgainSection(section)), "");
  renderMixShelf(homeEl, "Made from your listening", mixSeeds().slice(0, 4));
  renderTrackShelf(homeEl, "Heavy rotation", historyItems().slice(0, 6));
  renderTrackShelf(homeEl, "Recently played", recentHistoryItems().slice(0, 6));
  if (!homeEl.children.length) renderSections(homeEl, [], "No home sections");
}

function saveLastTrack(track) {
  if (!track?.id) return;
  localStorage.setItem("yolite:lastTrack", JSON.stringify(track));
}

function loadLastTrack() {
  try {
    return JSON.parse(localStorage.getItem("yolite:lastTrack") || "null");
  } catch {
    return null;
  }
}

function setNow(track) {
  currentTrack = track;
  if (track?.thumbnail) nowArt.src = track.thumbnail;
  else nowArt.removeAttribute("src");
  nowArt.toggleAttribute("data-empty", !track?.thumbnail);
  nowArt.addEventListener("error", event => {
    event.currentTarget.removeAttribute("src");
    event.currentTarget.toggleAttribute("data-empty", true);
  }, { once: true });
  nowTitle.textContent = track?.title || "Nothing playing";
  nowArtist.textContent = track?.artist || "Pick a track";
  duration.textContent = formatTime(track?.duration || 0);
  saveLastTrack(track);
  syncLikeButton();
  renderHome();
  renderDiscover();
  syncRemoteState();
}

function showPanel(panelId) {
  document.querySelectorAll(".nav").forEach(item => item.classList.toggle("active", item.dataset.panel === panelId));
  document.querySelectorAll(".panel").forEach(item => item.classList.toggle("active", item.id === panelId));
}

function stopNativeProgress() {
  clearInterval(nativeProgressTimer);
  nativeProgressTimer = 0;
}

async function updateNativeProgress() {
  if (!isTauri || !nativePlaying) {
    seek.value = "0";
    elapsed.textContent = "0:00";
    duration.textContent = formatTime(currentTrack?.duration || 0);
    return;
  }

  let state;
  try {
    state = await window.__TAURI__.core.invoke("get_native_playback");
  } catch {
    return;
  }
  nativePlaying = Boolean(state.playing);
  nativePaused = Boolean(state.paused);
  if (!nativePlaying) {
    stopNativeProgress();
    syncPlayButton();
    return;
  }

  const trackDuration = currentTrack?.duration || 0;
  const realDuration = state.duration || trackDuration;
  const position = state.position || 0;
  nativePosition = position;
  nativeDuration = realDuration;
  seek.value = realDuration ? String(Math.floor(position / realDuration * 1000)) : "0";
  elapsed.textContent = formatTime(position);
  duration.textContent = formatTime(realDuration);

  const fade = crossfadeSeconds();
  if (fade && !nativePaused && !crossfadeStarted && queue.length && currentIndex < queue.length - 1 && realDuration && realDuration - position <= fade) {
    crossfadeStarted = true;
    playNext("auto");
    return;
  }

  if (realDuration && position >= realDuration - 1) {
    stopNativeProgress();
    nativePlaying = false;
    nativePaused = false;
    syncPlayButton();
    playNext("auto");
  }
}

function startNativeProgress() {
  stopNativeProgress();
  updateNativeProgress();
  nativeProgressTimer = setInterval(updateNativeProgress, 1000);
}

function prefetchWindowSize() {
  return Math.max(0, Math.min(50, Number(prefetchCount.value) || 0));
}

function prefetchUpcomingTracks() {
  const count = prefetchWindowSize();
  if (!count) return;
  const upcoming = queue.slice(currentIndex + 1, currentIndex + 1 + count);
  for (const track of upcoming) {
    if (track?.id) api(`/api/prefetch/${encodeURIComponent(track.id)}`).catch(() => {});
  }
}

async function playTrack(track, options = {}) {
  if (!track) return;
  const requestId = ++playRequestId;
  resolving = true;
  crossfadeStarted = false;
  usingFallback = false;
  currentFallbackUrl = "";
  recordPlayHistory(track);
  setNow(track);
  nativePlaying = isTauri;
  nativePaused = false;
  nativePosition = 0;
  nativeDuration = track.duration || 0;
  seek.value = "0";
  elapsed.textContent = "0:00";
  syncPlayButton();
  setStatus(isTauri ? "Starting player" : "Resolving stream");
  let fallbackUrl = "";

  try {
    if (isTauri) {
      await window.__TAURI__.core.invoke("play_track_native", {
        videoId: track.id,
        volume: Number(volume.value),
        fadeIn: shouldFadeTransition(options.reason) ? crossfadeSeconds() : 0,
        equalizer: currentEqualizer(),
        visualizerEnabled: visualizerEnabled.checked && visualizerRenderer.available,
        title: track.title || "",
        artist: track.artist || ""
      });
      if (requestId !== playRequestId) return;
      nativePlaying = true;
      nativePaused = false;
      startNativeProgress();
      syncPlayButton();
      updateDiscordPresence(track, false);
      setStatus("Playing");
      prefetchUpcomingTracks();
      return;
    }

    const resolved = await api(`/api/resolve/${encodeURIComponent(track.id)}`);
    if (requestId !== playRequestId) return;
    fallbackUrl = resolved.fallbackUrl || "";
    currentFallbackUrl = fallbackUrl;
    const streamUrl = isTauri && fallbackUrl ? fallbackUrl : resolved.streamUrl || fallbackUrl;
    if (!streamUrl) throw new Error("No playable stream found");
    usingFallback = streamUrl === fallbackUrl;
    audio.src = streamUrl;
    if (audioContext?.state === "suspended") await audioContext.resume();
    await audio.play();
    updateDiscordPresence(track, false);
    setStatus("Playing");
    prefetchUpcomingTracks();
  } catch (error) {
    if (requestId !== playRequestId) return;
    if (!fallbackUrl) {
      setStatus(errorMessage(error, "Playback failed"));
      return;
    }
    await playFallback(fallbackUrl, error);
  } finally {
    if (requestId === playRequestId) {
      resolving = false;
      renderQueue();
    }
  }
}

async function playFallback(fallbackUrl, originalError) {
  setStatus("Trying proxy stream");
  usingFallback = true;
  audio.src = fallbackUrl;
  await audio.play()
    .then(() => setStatus("Playing"))
    .catch(playError => {
      setStatus(errorMessage(playError, errorMessage(originalError, "Playback failed")));
    });
}

function syncPlayButton() {
  const playing = isTauri ? nativePlaying && !nativePaused : !audio.paused;
  playShape.className = playing ? "pause-shape" : "play-shape";
  playBtn.setAttribute("aria-label", playing ? "Pause" : "Play");
  syncVisualizerAnimation();
}

function syncLoopButton() {
  const labels = {
    off: "Loop off",
    all: "Loop playlist",
    one: "Loop song"
  };
  loopBtn.classList.toggle("active", loopMode !== "off");
  loopBtn.setAttribute("aria-label", labels[loopMode]);
  loopBtn.setAttribute("title", labels[loopMode]);
  loopBtn.setAttribute("aria-pressed", String(loopMode !== "off"));
  loopBtn.querySelector(".loop-shape").toggleAttribute("data-one", loopMode === "one");
}

function cycleLoopMode() {
  loopMode = loopMode === "off" ? "all" : loopMode === "all" ? "one" : "off";
  localStorage.setItem("yolite:loopMode", loopMode);
  syncLoopButton();
  setStatus(loopBtn.getAttribute("aria-label"));
}

function updateDiscordPresence(track = currentTrack, paused = false) {
  if (!isTauri || !discordPresence?.checked || !track?.id) return Promise.resolve();
  return window.__TAURI__.core.invoke("update_discord_presence", {
    title: track.title || "Unknown song",
    artist: track.artist || "Unknown artist",
    videoId: track.id,
    thumbnail: track.thumbnail || "",
    paused
  }).catch(() => {});
}

function clearDiscordPresence() {
  if (!isTauri) return Promise.resolve();
  return window.__TAURI__.core.invoke("clear_discord_presence").catch(() => {});
}

function syncRemoteState() {
  if (!isTauri) return;
  window.__TAURI__.core.invoke("update_remote_state", {
    track: currentTrack ? {
      id: currentTrack.id || "",
      title: currentTrack.title || "Unknown song",
      artist: currentTrack.artist || "Unknown artist",
      thumbnail: currentTrack.thumbnail || ""
    } : null,
    playing: Boolean(nativePlaying && !nativePaused),
    volume: Number(volume.value)
  }).catch(() => {});
}

function browserVisualizerLevels() {
  if (!audioAnalyser || audio.paused) {
    return { bass: 0, mids: 0, treble: 0, energy: 0, peak: 0 };
  }
  if (!visualizerAnalyserData || visualizerAnalyserData.length !== audioAnalyser.frequencyBinCount) {
    visualizerAnalyserData = new Uint8Array(audioAnalyser.frequencyBinCount);
  }
  audioAnalyser.getByteFrequencyData(visualizerAnalyserData);

  const average = (start, end) => {
    let total = 0;
    for (let index = start; index < Math.min(end, visualizerAnalyserData.length); index += 1) {
      total += visualizerAnalyserData[index];
    }
    return total / (Math.max(1, Math.min(end, visualizerAnalyserData.length) - start) * 255);
  };
  let peak = 0;
  for (const value of visualizerAnalyserData) peak = Math.max(peak, value / 255);
  const bass = average(0, 9);
  const mids = average(9, 30);
  const treble = average(30, visualizerAnalyserData.length);
  return {
    bass,
    mids,
    treble,
    energy: Math.min(1, bass * 0.55 + mids * 0.32 + treble * 0.13),
    peak,
  };
}

function pollNativeVisualizer(time) {
  if (!isTauri || nativeVisualizerRequest || time - nativeVisualizerUpdatedAt < 80) return;
  nativeVisualizerRequest = true;
  nativeVisualizerUpdatedAt = time;
  window.__TAURI__.core.invoke("get_native_visualizer")
    .then(levels => {
      nativeVisualizerLevels = levels;
    })
    .catch(() => {
      nativeVisualizerLevels = { bass: 0, mids: 0, treble: 0, energy: 0, peak: 0 };
    })
    .finally(() => {
      nativeVisualizerRequest = false;
    });
}

function drawVisualizer(time) {
  visualizerFrame = requestAnimationFrame(drawVisualizer);
  if (time - visualizerLastFrame + 0.5 < 1000 / visualizerTargetFps) return;
  visualizerLastFrame = time;

  if (isTauri) pollNativeVisualizer(time);
  visualizerRenderer.render(time, isTauri ? nativeVisualizerLevels : browserVisualizerLevels());
}

function syncVisualizerAnimation() {
  const playing = isTauri ? nativePlaying && !nativePaused : !audio.paused;
  const active = visualizerRenderer.available
    && visualizerEnabled?.checked
    && !visualizer.hidden
    && playing
    && !document.hidden;
  if (active && !visualizerFrame) {
    visualizerLastFrame = 0;
    visualizerFrame = requestAnimationFrame(drawVisualizer);
  } else if (!active && visualizerFrame) {
    cancelAnimationFrame(visualizerFrame);
    visualizerFrame = 0;
    visualizerRenderer.clear();
  }
}

function hotkeySettings() {
  const stored = storedJson("yolite:hotkeys", defaultHotkeys);
  return { ...defaultHotkeys, ...stored };
}

function renderHotkeys() {
  const settings = hotkeySettings();
  hotkeyInputs.forEach(input => {
    input.value = settings[input.dataset.hotkey] || defaultHotkeys[input.dataset.hotkey];
  });
}

function shortcutFromEvent(event) {
  const parts = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Meta");
  let key = event.key;
  if (key === " ") key = "Space";
  if (key.length === 1) key = key.toUpperCase();
  if (!["Control", "Alt", "Shift", "Meta"].includes(key)) parts.push(key);
  return parts.join("+");
}

async function registerHotkeys() {
  const shortcuts = Object.fromEntries(hotkeyInputs.map(input => [input.dataset.hotkey, input.value.trim()]));
  localStorage.setItem("yolite:hotkeys", JSON.stringify(shortcuts));
  if (!isTauri) {
    hotkeyState.textContent = "Hotkeys work while browser tab is focused.";
    return;
  }
  try {
    await window.__TAURI__.core.invoke("set_global_shortcuts", { shortcuts });
    hotkeyState.textContent = "Global hotkeys active.";
  } catch (error) {
    hotkeyState.textContent = errorMessage(error, "Could not register hotkeys");
  }
}

async function handleControlAction(action) {
  if (action === "playPause") return togglePlayback();
  if (action === "next") return playNext("manual");
  if (action === "previous") return playPrev();
  if (action === "volumeUp") return applyVolume(Math.min(1, Number(volume.value) + 0.05));
  if (action === "volumeDown") return applyVolume(Math.max(0, Number(volume.value) - 0.05));
}

async function toggleMiniPlayer() {
  if (!isTauri) return;
  miniPlayer = !miniPlayer;
  try {
    await window.__TAURI__.core.invoke("set_mini_player", { enabled: miniPlayer });
    document.body.classList.toggle("mini-player", miniPlayer);
    miniPlayerButton.classList.toggle("active", miniPlayer);
  } catch (error) {
    miniPlayer = !miniPlayer;
    setStatus(errorMessage(error, "Could not open mini player"));
  }
}

async function showRemoteController() {
  if (!isTauri) return;
  remoteButton.disabled = true;
  try {
    const payload = await window.__TAURI__.core.invoke("get_remote_controller");
    remoteQr.innerHTML = payload.qrSvg;
    remoteUrl.href = payload.url;
    remoteUrl.textContent = payload.url;
    remoteDialog.showModal?.();
  } catch (error) {
    setStatus(errorMessage(error, "Could not start phone controller"));
  } finally {
    remoteButton.disabled = false;
  }
}

async function loadPlugins() {
  pluginList.textContent = "";
  pluginCategories = [];
  document.querySelectorAll("style[data-yolite-plugin]").forEach(style => style.remove());
  if (!isTauri) {
    pluginList.textContent = "Plugins load in desktop app";
    return;
  }
  try {
    const plugins = await window.__TAURI__.core.invoke("load_plugins");
    plugins.forEach(plugin => {
      const item = document.createElement("div");
      item.className = "plugin-item";
      item.innerHTML = "<strong></strong><span></span><code></code>";
      item.querySelector("strong").textContent = plugin.name;
      item.querySelector("span").textContent = plugin.description || "YoLite plugin";
      item.querySelector("code").textContent = plugin.version || "1.0.0";
      pluginList.append(item);
      if (plugin.css) {
        const style = document.createElement("style");
        style.dataset.yolitePlugin = plugin.id;
        style.textContent = plugin.css;
        document.head.append(style);
      }
      pluginCategories.push(...(plugin.discoverCategories || []));
    });
    if (!plugins.length) pluginList.textContent = "No plugins installed";
    renderDiscover();
  } catch (error) {
    pluginList.textContent = errorMessage(error, "Could not load plugins");
  }
}

async function playNext(reason = "manual") {
  if (!queue.length) return;
  if (reason === "auto" && loopMode === "one") {
    playTrack(queue[currentIndex], { reason });
    return;
  }
  const nextIndex = currentIndex + 1;
  if (nextIndex >= queue.length) {
    if (loopMode === "all") {
      currentIndex = 0;
    } else {
      const seed = queue[currentIndex] || currentTrack;
      const extension = await fetchTrackMix(seed);
      const existing = new Set(queue.map(item => item.id));
      queue.push(...extension.filter(item => !existing.has(item.id)));
      renderQueue();
      if (currentIndex + 1 >= queue.length) {
        nativePlaying = false;
        nativePaused = false;
        syncPlayButton();
        clearDiscordPresence();
        return;
      }
      currentIndex += 1;
    }
  } else {
    currentIndex = nextIndex;
  }
  playTrack(queue[currentIndex], { reason });
}

async function toggleLike() {
  if (!currentTrack?.id) return;
  const nextLiked = !likedTracks.has(currentTrack.id);
  if (nextLiked) likedTracks.add(currentTrack.id);
  else likedTracks.delete(currentTrack.id);
  syncLikeButton();

  await api("/api/like", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ videoId: currentTrack.id, liked: nextLiked })
  }).then(() => {
    setStatus(nextLiked ? "Liked" : "Removed like");
  }).catch(error => {
    if (nextLiked) likedTracks.delete(currentTrack.id);
    else likedTracks.add(currentTrack.id);
    syncLikeButton();
    setStatus(errorMessage(error, "Could not update like"));
  });
}

function openPlaylistDialog(focusCreate = false, track = currentTrack) {
  playlistDialogTrack = track;
  renderPlaylistSelect();
  saveToPlaylist.disabled = !playlistDialogTrack?.id || !playlists.length;
  if (playlistDialog.showModal) playlistDialog.showModal();
  else playlistDialog.setAttribute("open", "");
  if (focusCreate) newPlaylistName.focus();
}

async function createPlaylistFromDialog() {
  const title = newPlaylistName.value.trim();
  if (!title) {
    setStatus("Name the playlist first");
    return;
  }
  createPlaylist.disabled = true;
  try {
    const payload = await api("/api/playlist", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ title })
    });
    const playlist = payload.playlist || payload;
    playlists.unshift(playlist);
    newPlaylistName.value = "";
    renderSidebarPlaylists();
    renderLibrary();
    playlistSelect.value = playlist.id;
    setStatus("Playlist created");
  } catch (error) {
    setStatus(errorMessage(error, "Could not create playlist"));
  } finally {
    createPlaylist.disabled = false;
  }
}

async function saveCurrentTrackToPlaylist() {
  const playlistId = playlistSelect.value;
  if (!playlistDialogTrack?.id || !playlistId) return;
  saveToPlaylist.disabled = true;
  try {
    await api(`/api/playlist/${encodeURIComponent(playlistId)}/tracks`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ videoId: playlistDialogTrack.id })
    });
    setStatus("Saved to playlist");
    playlistDialog.close?.();
  } catch (error) {
    setStatus(errorMessage(error, "Could not save to playlist"));
  } finally {
    saveToPlaylist.disabled = false;
  }
}

async function playPrev() {
  if (!queue.length) return;
  if (isTauri) {
    if (nativePosition > 3) {
      await window.__TAURI__.core.invoke("seek_native_playback", { position: 0 }).catch(error => {
        setStatus(errorMessage(error, "Could not seek"));
      });
      nativePosition = 0;
      return;
    }
    currentIndex = Math.max(0, currentIndex - 1);
    playTrack(queue[currentIndex], { reason: "manual" });
    return;
  }
  if (audio.currentTime > 3) {
    audio.currentTime = 0;
    return;
  }
  currentIndex = Math.max(0, currentIndex - 1);
  playTrack(queue[currentIndex], { reason: "manual" });
}

searchForm.addEventListener("submit", async event => {
  event.preventDefault();
  const query = searchInput.value.trim();
  if (query.length < 2) return;
  setStatus("Searching");
  resultsEl.textContent = "";
  try {
    const payload = await api(`/api/search?q=${encodeURIComponent(query)}`);
    results = payload.results;
    renderResults();
    showPanel("searchPanel");
    setStatus(results.length ? `${results.length} results` : "No results");
  } catch (error) {
    setStatus(errorMessage(error));
  }
});

document.querySelectorAll(".nav").forEach(button => {
  button.addEventListener("click", () => {
    showPanel(button.dataset.panel);
    if (button.dataset.panel === "homePanel" && !homeSections.length) {
      loadHome();
    }
    if (button.dataset.panel === "libraryPanel" && !librarySections.length) {
      loadLibrary();
    }
    if (button.dataset.panel === "discoverPanel" && !homeSections.length) {
      loadHome();
    }
  });
});

async function togglePlayback() {
  if (!currentTrack && queue.length) {
    currentIndex = Math.max(currentIndex, 0);
    playTrack(queue[currentIndex], { reason: "manual" });
    return;
  }
  if (isTauri) {
    if (nativePlaying) {
      await window.__TAURI__.core.invoke("set_native_pause", { paused: !nativePaused }).catch(error => {
        setStatus(errorMessage(error, "Could not change playback"));
      });
      nativePaused = !nativePaused;
      syncPlayButton();
      updateDiscordPresence(currentTrack, nativePaused);
      syncRemoteState();
      setStatus(nativePaused ? "Paused" : "Playing");
    } else if (currentTrack) {
      playTrack(currentTrack, { reason: "manual" });
    }
    return;
  }
  if (audio.paused) audio.play();
  else audio.pause();
}

playBtn.addEventListener("click", togglePlayback);

prevBtn.addEventListener("click", playPrev);
nextBtn.addEventListener("click", playNext);
loopBtn.addEventListener("click", cycleLoopMode);
queueToggle.addEventListener("click", () => {
  setQueueOpen(queueDrawer.hidden);
});
likeBtn.addEventListener("click", toggleLike);
saveTrackBtn.addEventListener("click", () => openPlaylistDialog(false));
newPlaylist.addEventListener("click", () => openPlaylistDialog(true));
closePlaylistDialog.addEventListener("click", () => playlistDialog.close?.());
trackMenu.addEventListener("click", async event => {
  const action = event.target.closest("[data-menu-action]")?.dataset.menuAction;
  const track = contextTrack;
  closeTrackMenu();
  if (!action || !track) return;
  if (action === "next") {
    playTrackNext(track);
    return;
  }
  if (action === "playlist") {
    openPlaylistDialog(false, track);
    return;
  }
  if (action === "mix") {
    try {
      await startYouTubeMix(track);
    } catch (error) {
      setStatus(errorMessage(error, "Could not start mix"));
    }
  }
});
document.addEventListener("pointerdown", event => {
  if (!trackMenu.hidden && !trackMenu.contains(event.target)) closeTrackMenu();
  if (!profileMenu.hidden && !profileMenu.contains(event.target) && !profileButton.contains(event.target)) {
    profileMenu.hidden = true;
    profileButton.setAttribute("aria-expanded", "false");
  }
});
document.addEventListener("keydown", event => {
  if (event.key === "Escape") closeTrackMenu();
  if (!/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || "")) {
    const shortcut = shortcutFromEvent(event);
    const action = Object.entries(hotkeySettings()).find(([, value]) => value === shortcut)?.[0];
    if (action) {
      event.preventDefault();
      handleControlAction(action);
      return;
    }
  }
  if (event.key === "/" && !/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || "")) {
    event.preventDefault();
    searchInput.focus();
  }
});
profileButton.addEventListener("click", () => {
  profileMenu.hidden = !profileMenu.hidden;
  profileButton.setAttribute("aria-expanded", String(!profileMenu.hidden));
});
profilePicture.addEventListener("error", () => {
  profilePicture.removeAttribute("src");
  profileFallback.hidden = false;
});
profileSettings.addEventListener("click", () => {
  profileMenu.hidden = true;
  showPanel("settingsPanel");
});
saveToPlaylist.addEventListener("click", event => {
  event.preventDefault();
  saveCurrentTrackToPlaylist();
});
createPlaylist.addEventListener("click", event => {
  event.preventDefault();
  createPlaylistFromDialog();
});
refreshDiscover.addEventListener("click", loadHome);
settingsVolume.addEventListener("input", () => applyVolume(settingsVolume.value));
volume.addEventListener("input", () => applyVolume(volume.value));
settingsCrossfade.addEventListener("input", () => applyCrossfade(settingsCrossfade.value));
crossfadeMode.addEventListener("change", () => saveCrossfadeMode(crossfadeMode.value));
mixLength.addEventListener("input", () => {
  localStorage.setItem("yolite:mixLength", mixLength.value);
  updateSettingsLabels();
});
prefetchCount.addEventListener("input", () => {
  localStorage.setItem("yolite:prefetchCount", prefetchCount.value);
  updateSettingsLabels();
  prefetchUpcomingTracks();
});
cacheLimit.addEventListener("input", () => {
  localStorage.setItem("yolite:cacheLimitGb", cacheLimit.value);
  updateSettingsLabels();
});
playlistWarnLimit.addEventListener("input", () => {
  localStorage.setItem("yolite:playlistWarnLimit", playlistWarnLimit.value);
  updateSettingsLabels();
});
discoverFromHistory.addEventListener("change", () => {
  localStorage.setItem("yolite:discoverFromHistory", JSON.stringify(discoverFromHistory.checked));
  renderHome();
  renderDiscover();
});
discordPresence?.addEventListener("change", () => {
  localStorage.setItem("yolite:discordPresence", JSON.stringify(discordPresence.checked));
  if (discordPresence.checked) updateDiscordPresence();
  else clearDiscordPresence();
});
volumeNormalization.addEventListener("change", () => {
  localStorage.setItem("yolite:volumeNormalization", JSON.stringify(volumeNormalization.checked));
  applyEqualizer();
});
visualizerEnabled.addEventListener("change", () => {
  localStorage.setItem("yolite:visualizer", JSON.stringify(visualizerEnabled.checked));
  visualizer.hidden = !visualizerEnabled.checked;
  if (isTauri) {
    const enabled = visualizerEnabled.checked && visualizerRenderer.available;
    window.__TAURI__.core.invoke("set_native_visualizer", { enabled }).catch(() => {});
  }
  syncVisualizerAnimation();
});
visualizerFps.addEventListener("change", () => applyVisualizerSettings(visualizerSettings()));
[visualizerBassColor, visualizerMidsColor, visualizerTrebleColor].forEach(input => {
  input.addEventListener("input", () => applyVisualizerSettings(visualizerSettings()));
});
resetVisualizer.addEventListener("click", () => applyVisualizerSettings(defaultVisualizerSettings));
automaticUpdateChecks.addEventListener("change", () => {
  localStorage.setItem("yolite:automaticUpdateChecks", JSON.stringify(automaticUpdateChecks.checked));
  settingsState.textContent = "Saved locally";
});
checkForUpdateButton.addEventListener("click", () => checkForUpdates());
installUpdateButton.addEventListener("click", installAvailableUpdate);
document.addEventListener("visibilitychange", syncVisualizerAnimation);
hotkeyInputs.forEach(input => input.addEventListener("change", registerHotkeys));
resetHotkeys.addEventListener("click", () => {
  localStorage.setItem("yolite:hotkeys", JSON.stringify(defaultHotkeys));
  renderHotkeys();
  registerHotkeys();
});
miniPlayerButton.addEventListener("click", toggleMiniPlayer);
remoteButton.addEventListener("click", showRemoteController);
reloadPlugins.addEventListener("click", loadPlugins);
resetEqualizer.addEventListener("click", () => setEqualizer(equalizerPresets.flat));
presetButtons.forEach(button => {
  button.addEventListener("click", () => setEqualizer(equalizerPresets[button.dataset.preset] || equalizerPresets.flat));
});
eqControls.forEach(control => {
  control.addEventListener("input", () => {
    updateSettingsLabels();
    localStorage.setItem("yolite:equalizer", JSON.stringify(currentEqualizer()));
    applyEqualizer();
  });
});
clearQueue.addEventListener("click", () => {
  queue = [];
  currentIndex = -1;
  renderQueue();
});

seek.addEventListener("input", () => {
  if (isTauri) {
    const target = (nativeDuration || currentTrack?.duration || 0) * Number(seek.value) / 1000;
    window.__TAURI__.core.invoke("seek_native_playback", { position: target }).catch(error => {
      setStatus(errorMessage(error, "Could not seek"));
    });
    nativePosition = target;
    elapsed.textContent = formatTime(target);
    return;
  }
  if (!audio.duration) return;
  audio.currentTime = Number(seek.value) / 1000 * audio.duration;
});

audio.addEventListener("play", () => {
  syncPlayButton();
  syncRemoteState();
});
audio.addEventListener("pause", () => {
  syncPlayButton();
  syncRemoteState();
});
audio.addEventListener("ended", () => playNext("auto"));
audio.addEventListener("timeupdate", () => {
  const knownDuration = audio.duration || currentTrack?.duration || 0;
  seek.value = knownDuration ? String(Math.floor(audio.currentTime / knownDuration * 1000)) : "0";
  elapsed.textContent = formatTime(audio.currentTime);
  duration.textContent = formatTime(knownDuration);
});
audio.addEventListener("error", () => {
  if (currentTrack && currentFallbackUrl && !usingFallback) {
    playFallback(currentFallbackUrl, new Error("Direct stream failed"));
    return;
  }
  setStatus("Playback failed");
});

saveCookies.addEventListener("click", async () => {
  try {
    const payload = await api("/api/session", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ cookies: cookieInput.value, userId: userIdInput.value, channelId: channelIdInput.value })
    });
    cookieInput.value = "";
    applySession(payload);
    librarySections = [];
    libraryNeedsLogin = false;
    playlists = [];
    activePlaylist = null;
    activePlaylistTracks = [];
    loadLibrary();
  } catch (error) {
    sessionState.textContent = errorMessage(error);
  }
});

openYoutubeMusic.addEventListener("click", async event => {
  if (!isTauri) return;
  event.preventDefault();
  try {
    await window.__TAURI__.core.invoke("open_youtube_music");
  } catch (error) {
    sessionState.textContent = errorMessage(error, "Could not open browser");
  }
});

importBrowserCookies.addEventListener("click", async () => {
  importBrowserCookies.disabled = true;
  sessionState.textContent = browserSource.value === "auto"
    ? "Finding YouTube Music cookies"
    : `Importing from ${browserSource.options[browserSource.selectedIndex].text}`;
  try {
    const payload = await api("/api/session/import", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ browser: browserSource.value, userId: userIdInput.value, channelId: channelIdInput.value })
    });
    applySession(payload);
    librarySections = [];
    libraryNeedsLogin = false;
    playlists = [];
    activePlaylist = null;
    activePlaylistTracks = [];
    loadLibrary();
  } catch (error) {
    sessionState.textContent = errorMessage(error);
  } finally {
    importBrowserCookies.disabled = false;
  }
});

async function importInAppLogin(silent = false) {
  if (!silent) useAppLogin.disabled = true;
  if (!silent) sessionState.textContent = "Reading in-app YouTube Music login";
  try {
    const payload = await api("/api/session/app-login/import", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ userId: "", channelId: channelIdInput.value })
    });
    cookieInput.value = "";
    applySession(payload);
    librarySections = [];
    libraryNeedsLogin = false;
    playlists = [];
    activePlaylist = null;
    activePlaylistTracks = [];
    loadLibrary();
    return true;
  } catch (error) {
    if (!silent) sessionState.textContent = errorMessage(error);
    return false;
  } finally {
    if (!silent) useAppLogin.disabled = false;
  }
}

async function pollInAppLogin(deadline = Date.now() + 5 * 60 * 1000) {
  clearTimeout(loginPollTimer);
  const imported = await importInAppLogin(true);
  if (imported || Date.now() >= deadline) {
    if (!imported) sessionState.textContent = "Login window timed out";
    return;
  }
  loginPollTimer = setTimeout(() => pollInAppLogin(deadline), 2500);
}

async function beginInAppLogin() {
  openAppLogin.disabled = true;
  sessionState.textContent = "Opening in-app login";
  try {
    await api("/api/session/app-login/open", { method: "POST" });
    sessionState.textContent = "Waiting for YouTube Music login";
    pollInAppLogin();
  } catch (error) {
    sessionState.textContent = errorMessage(error, "Could not open in-app login");
  } finally {
    openAppLogin.disabled = false;
  }
}

openAppLogin.addEventListener("click", beginInAppLogin);
profileLogin.addEventListener("click", beginInAppLogin);

useAppLogin.addEventListener("click", () => importInAppLogin(false));

clearCookies.addEventListener("click", async () => {
  const payload = await api("/api/session", { method: "DELETE" });
  applySession(payload);
  librarySections = [];
  libraryNeedsLogin = false;
  playlists = [];
  activePlaylist = null;
  activePlaylistTracks = [];
  renderLibrary();
  renderSidebarPlaylists();
});

async function loadSession() {
  const payload = await api("/api/session");
  applySession(payload);
}

async function loadLibrary() {
  activePlaylist = null;
  activePlaylistTracks = [];
  libraryEl.textContent = "";
  const loading = document.createElement("div");
  loading.className = "empty";
  loading.textContent = "Loading library";
  libraryEl.append(loading);

  try {
    const payload = await api("/api/library");
    libraryNeedsLogin = Boolean(payload.needsLogin);
    librarySections = libraryNeedsLogin ? [] : payload.sections || [];
    playlists = libraryNeedsLogin ? [] : payload.playlists || [];
    syncLikedTracksFromLibrary();
    renderLibrary();
    renderSidebarPlaylists();
  } catch (error) {
    libraryEl.textContent = "";
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = errorMessage(error);
    libraryEl.append(empty);
  }
}

async function openPlaylist(playlist) {
  if (!playlist?.id) return;
  showPanel("libraryPanel");
  activePlaylist = playlist;
  const cached = loadCachedPlaylist(playlist.id);
  activePlaylistTracks = cached?.tracks || [];
  renderSidebarPlaylists();
  if (activePlaylistTracks.length) {
    renderLibrary();
    setStatus(`Checking ${playlist.title}`);
  } else {
    libraryEl.textContent = "";
    const loading = document.createElement("div");
    loading.className = "empty";
    loading.textContent = `Loading ${playlist.title}`;
    libraryEl.append(loading);
  }

  try {
    const payload = await api(`/api/playlist/${encodeURIComponent(playlist.id)}`);
    libraryNeedsLogin = false;
    const freshTracks = payload.tracks || [];
    const changeText = activePlaylistTracks.length ? describePlaylistChanges(activePlaylistTracks, freshTracks) : "";
    activePlaylist = playlist;
    activePlaylistTracks = freshTracks;
    saveCachedPlaylist(playlist.id, activePlaylistTracks);
    renderLibrary();
    renderSidebarPlaylists();
    setStatus(changeText ? `Playlist updated: ${changeText}` : "Playlist up to date");
  } catch (error) {
    if (activePlaylistTracks.length) {
      setStatus(errorMessage(error, "Could not refresh playlist"));
      return;
    }
    libraryEl.textContent = "";
    const back = document.createElement("button");
    back.className = "playlist-back ghost";
    back.type = "button";
    back.textContent = "Back to Library";
    back.addEventListener("click", () => {
      activePlaylist = null;
      activePlaylistTracks = [];
      renderLibrary();
      renderSidebarPlaylists();
    });
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = errorMessage(error);
    libraryEl.append(back, empty);
  }
}

refreshLibrary.addEventListener("click", loadLibrary);

applyVolume(localStorage.getItem("yolite:volume") || volume.value);
applyCrossfade(localStorage.getItem("yolite:crossfade") || settingsCrossfade.value);
saveCrossfadeMode(localStorage.getItem("yolite:crossfadeMode") || crossfadeMode.value);
mixLength.value = localStorage.getItem("yolite:mixLength") || mixLength.value;
prefetchCount.value = localStorage.getItem("yolite:prefetchCount") || prefetchCount.value;
cacheLimit.value = localStorage.getItem("yolite:cacheLimitGb") || cacheLimit.value;
playlistWarnLimit.value = localStorage.getItem("yolite:playlistWarnLimit") || playlistWarnLimit.value;
discoverFromHistory.checked = storedJson("yolite:discoverFromHistory", true);
discordPresence.checked = storedJson("yolite:discordPresence", true);
volumeNormalization.checked = storedJson("yolite:volumeNormalization", false);
visualizerEnabled.checked = storedJson("yolite:visualizer", true);
automaticUpdateChecks.checked = storedJson("yolite:automaticUpdateChecks", true);
visualizer.hidden = !visualizerEnabled.checked;
applyVisualizerSettings(storedJson("yolite:visualizerSettings", defaultVisualizerSettings), false);
loopMode = ["off", "all", "one"].includes(localStorage.getItem("yolite:loopMode"))
  ? localStorage.getItem("yolite:loopMode")
  : "off";
setEqualizer(storedJson("yolite:equalizer", equalizerPresets.flat));
updateSettingsLabels();
renderResults();
renderHistory();
syncLoopButton();
renderHotkeys();
syncVisualizerAnimation();

function hideSplash() {
  splash.classList.add("done");
  setTimeout(() => splash.remove(), 360);
}

async function loadHome() {
  homeEl.textContent = "";
  const loading = document.createElement("div");
  loading.className = "empty";
  loading.textContent = "Loading home";
  homeEl.append(loading);

  try {
    const payload = await api("/api/home");
    homeSections = payload.sections || [];
    renderHome();
    renderDiscover();
    hideSplash();
  } catch (error) {
    homeSections = [];
    renderSections(homeEl, [], errorMessage(error));
    renderDiscover();
    hideSplash();
  }
}

searchChips.addEventListener("click", event => {
  const chip = event.target.closest(".chip");
  if (!chip) return;
  activeSearchScope = chip.dataset.scope || "all";
  searchChips.querySelectorAll(".chip").forEach(item => item.classList.toggle("active", item === chip));
  renderResults();
});

loadHome();
loadLibrary();
renderDiscover();
renderSidebarPlaylists();
renderQueue();
setQueueOpen(false);
syncPlayButton();
registerHotkeys();
loadPlugins();
setTimeout(hideSplash, 1400);
if (isTauri && automaticUpdateChecks.checked) setTimeout(() => checkForUpdates({ quiet: true }), 2500);
if (!isTauri) checkForUpdates({ quiet: true });
if (!isTauri) {
  appLoginActions.hidden = true;
  appLoginHelp.hidden = true;
  discordPresenceRow.hidden = true;
  remoteButton.hidden = true;
  miniPlayerButton.hidden = true;
  profileLogin.hidden = true;
}
loadSession().catch(error => {
  sessionState.textContent = errorMessage(error);
});

if (isTauri && window.__TAURI__?.event?.listen) {
  window.__TAURI__.event.listen("global-hotkey", event => handleControlAction(event.payload));
  window.__TAURI__.event.listen("remote-control", event => handleControlAction(event.payload));
}

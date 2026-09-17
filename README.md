# Yolite

A lightweight YouTube Music client that avoids Electron. It can run as a native Tauri/WebKitGTK desktop app, with a local browser prototype kept for quick testing.

## What Works

- YouTube Music song search through `ytmusic-api`
- Native desktop shell through Tauri/WebKitGTK
- Node-free desktop backend through `rs-ytmusic-api`
- Cookie-backed account session storage for logged-in Library and age-restricted playback
- Library tab with Liked Music and personalized YouTube Music sections when cookies are saved
- Audio playback through `yt-dlp`
- Desktop loopback stream fallback when direct media URLs fail in the webview
- Queue, previous/next, progress, seek, volume, and compact responsive UI
- Global configurable playback hotkeys and compact always-on-top mini player
- Token-protected phone controller with current-song download over the local network
- Declarative theme and discovery plugins
- Linux-first, with a Windows-capable Tauri path

## Requirements

- Node.js 22+
- Python 3.9+ for `youtube-dl-exec` install/update support

The `youtube-dl-exec` package installs a small `yt-dlp` binary under `node_modules/youtube-dl-exec/bin`.

## Run

### Desktop App

```bash
npm install
npm run desktop:dev
```

That launches the Tauri desktop app. On Linux it uses WebKitGTK, not Firefox, Chromium, or Electron.

Wayland is the default target. The launcher disables WebKitGTK's DMA-BUF renderer on Wayland because that path can crash on some GPU drivers while opening the window.

Build a native package with:

```bash
npm run desktop:build
```

### Local Browser Prototype

```bash
npm install
npm start
```

Then open the printed local URL.

## Project structure

The desktop backend is split by ownership so changes stay focused and reviewable:

- `src-tauri/src/lib.rs` composes the Tauri application and registers commands.
- `src-tauri/src/models.rs` contains shared command payloads and application state.
- `src-tauri/src/config.rs` owns local configuration and cookie normalization.
- `src-tauri/src/youtube.rs` owns YouTube Music requests, parsing, and account commands.
- `src-tauri/src/playback.rs` owns `mpv`, `yt-dlp`, equalizer, and visualizer telemetry.
- `src-tauri/src/servers.rs` owns loopback streaming and the phone controller server.
- `src-tauri/src/desktop.rs`, `discord.rs`, `plugins.rs`, and `remote.rs` contain their named integrations.
- `public/app.js` coordinates the interface. `public/visualizer.js` contains the WebGL renderer.

Keep new code in the module that owns its state or external boundary. Add shared command payloads to `models.rs`; keep `lib.rs` limited to application composition.

Run both test suites before opening a pull request:

```bash
(cd src-tauri && cargo test)
npm run test:e2e
```

## Account Login

Open YouTube Music in your normal browser and sign in there first. In Yolite's Session panel, choose that browser and use Import Browser Cookies. The app asks `yt-dlp` to read the browser profile and stores the resulting YouTube cookies locally.

If browser import cannot read your profile, export or copy your YouTube cookie header and paste it manually in the same Session panel. After cookies are saved, the Library tab can load account-backed content such as Liked Music.

In the desktop app, you can also use Open In-App Login from the Session panel. Sign in or switch to the YouTube account you use for music in that window, then choose Use In-App Login to save that webview session.

If one Gmail login owns multiple YouTube accounts, switch to the account you use for music and open `https://www.youtube.com/account_advanced` to confirm the active YouTube account. Use the Channel ID from that page to include public YouTube playlists from that channel. If Yolite still needs an explicit Music account selector, use the numeric Brand Account ID from the `/b/<id>/` URL at `https://myaccount.google.com/brandaccounts`, then save or import the session again.

Accepted formats:

- A normal `Cookie` header: `SID=...; HSID=...; SSID=...`
- Netscape cookies.txt export lines

Cookies are stored at:

```text
~/.config/yolite/config.json
```

Set `YOLITE_CONFIG=/path/to/config.json` to use a different config file.

## Notes

This is intentionally not an Electron app. The desktop path uses Tauri/Wry with the system WebKitGTK webview on Linux.

The desktop path uses the MIT-licensed Rust `rs-ytmusic-api` crate for YouTube Music search. The local browser prototype still uses the GPL-3.0 `ytmusic-api` npm package.

## Plugins and themes

YoLite loads desktop plugins from `~/.config/yolite/plugins`. Each plugin uses its own directory:

```text
plugins/
  warm-studio/
    plugin.json
    theme.css
```

`plugin.json`:

```json
{
  "name": "Warm studio",
  "version": "1.0.0",
  "description": "Warm neutral surfaces and extra focus categories",
  "discoverCategories": ["Deep focus", "Late-night jazz"]
}
```

`theme.css` can override YoLite CSS variables and component styles. Plugins are local and declarative; they do not execute JavaScript. Only install themes from sources you trust because CSS changes the app interface.

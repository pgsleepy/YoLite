use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::process::Stdio;
use std::sync::Arc;
use std::thread;

use ring::rand::{SecureRandom, SystemRandom};
use tauri::Emitter;

use crate::{config::*, models::*, playback::*, youtube::valid_video_id};

pub(crate) fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = [0; 2048];
    let mut request = Vec::new();

    loop {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") || request.len() > 8192 {
            break;
        }
    }

    String::from_utf8(request).ok()
}

pub(crate) fn http_text(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-store\r\naccess-control-allow-origin: *\r\n\r\n{body}",
        body.len()
    );
}

pub(crate) fn handle_stream_request(mut stream: TcpStream, app: tauri::AppHandle) {
    let Some(request) = read_http_request(&mut stream) else {
        return;
    };
    let Some(first_line) = request.lines().next() else {
        return;
    };
    let parts = first_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 || parts[0] != "GET" {
        return http_text(&mut stream, "405 Method Not Allowed", "Method not allowed");
    }

    let raw_path = parts[1];
    let path = raw_path.split('?').next().unwrap_or("");
    let use_cookies = raw_path
        .split_once('?')
        .map(|(_, query)| query.split('&').any(|part| part == "auth=1"))
        .unwrap_or(false);
    let Some(video_id) = path.strip_prefix("/stream/") else {
        return http_text(&mut stream, "404 Not Found", "Not found");
    };
    if !valid_video_id(video_id) {
        return http_text(&mut stream, "400 Bad Request", "Invalid video id");
    }

    let ytdlp = match ytdlp_path(&app) {
        Ok(path) => path,
        Err(error) => {
            return http_text(
                &mut stream,
                "502 Bad Gateway",
                &format!("Failed to start yt-dlp: {error}"),
            );
        }
    };

    let first_cookie_file = if use_cookies {
        let config = read_config();
        match TempCookieFile::new(&config.cookies) {
            Ok(cookie_file) => cookie_file,
            Err(error) => {
                return http_text(
                    &mut stream,
                    "502 Bad Gateway",
                    &format!("Could not prepare cookies: {error}"),
                );
            }
        }
    } else {
        None
    };
    let first_cookie_path = first_cookie_file
        .as_ref()
        .map(|cookie_file| cookie_file.path.as_path());
    let mut child = match external_command(&ytdlp)
        .args(ytdlp_stream_args(video_id, first_cookie_path))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return http_text(
                &mut stream,
                "502 Bad Gateway",
                &format!("Failed to start yt-dlp: {error}"),
            );
        }
    };

    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        return http_text(&mut stream, "502 Bad Gateway", "No yt-dlp output");
    };

    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\ncontent-type: audio/mp4\r\ncache-control: no-store\r\naccess-control-allow-origin: *\r\nconnection: close\r\n\r\n",
    );
    let copied = std::io::copy(&mut stdout, &mut stream).unwrap_or(0);
    let _ = child.wait();

    if copied > 0 {
        return;
    }
    if use_cookies {
        return;
    }

    let config = read_config();
    let Some(cookie_file) = TempCookieFile::new(&config.cookies).ok().flatten() else {
        return;
    };
    let Ok(mut child) = external_command(&ytdlp)
        .args(ytdlp_stream_args(video_id, Some(&cookie_file.path)))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    if let Some(mut stdout) = child.stdout.take() {
        let _ = std::io::copy(&mut stdout, &mut stream);
    }
    let _ = child.wait();
}

pub(crate) fn start_stream_server(app: tauri::AppHandle) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            thread::spawn(move || handle_stream_request(stream, app));
        }
    });
    Ok(format!("http://{addr}"))
}

pub(crate) fn secure_token() -> Result<String, String> {
    let mut bytes = [0_u8; 18];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Could not create controller token".to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn local_network_ip() -> Result<String, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    socket
        .connect("8.8.8.8:80")
        .map_err(|error| error.to_string())?;
    socket
        .local_addr()
        .map(|address| address.ip().to_string())
        .map_err(|error| error.to_string())
}

pub(crate) fn http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}

pub(crate) fn controller_html(token: &str) -> String {
    r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>YoLite remote</title>
<style>
:root{color-scheme:dark;font-family:system-ui,sans-serif}*{box-sizing:border-box}body{margin:0;min-height:100dvh;display:grid;place-items:center;background:#0b0c0d;color:#f4f1eb;padding:22px}.card{width:min(430px,100%);padding:22px;border:1px solid #ffffff18;border-radius:22px;background:#15171ae8;box-shadow:0 26px 80px #0008}.now{display:grid;grid-template-columns:82px 1fr;gap:16px;align-items:center}.now img{width:82px;height:82px;border-radius:14px;object-fit:cover;background:#24282c}.muted{color:#aaa7a1}.controls{display:grid;grid-template-columns:repeat(3,1fr);gap:10px;margin:22px 0 12px}button,a{min-height:52px;border:0;border-radius:13px;background:#24282c;color:#f4f1eb;font:700 16px inherit;text-decoration:none;display:grid;place-items:center}button.primary{background:#f4f1eb;color:#111315}.secondary{display:grid;grid-template-columns:1fr 1fr;gap:10px}.download{margin-top:10px;background:#d38a64;color:#111315}h1{font-size:17px;margin:0 0 18px}strong{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:18px}.muted{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;margin-top:4px}
</style></head>
<body><main class="card"><h1>YoLite remote</h1><section class="now"><img id="art" alt=""><div><strong id="title">Nothing playing</strong><div id="artist" class="muted">Pick a song in YoLite</div></div></section><div class="controls"><button data-action="previous">Previous</button><button class="primary" data-action="playPause">Play / pause</button><button data-action="next">Next</button></div><div class="secondary"><button data-action="volumeDown">Volume down</button><button data-action="volumeUp">Volume up</button></div><a id="download" class="download" hidden>Download song</a></main><script>
pub(crate) const token="__TOKEN__";const title=document.querySelector('#title');const artist=document.querySelector('#artist');const art=document.querySelector('#art');const download=document.querySelector('#download');document.querySelectorAll('[data-action]').forEach(button=>button.onclick=()=>fetch(`/api/control/${token}/${button.dataset.action}`));async function update(){try{const state=await fetch(`/api/state/${token}`).then(response=>response.json());title.textContent=state.track?.title||'Nothing playing';artist.textContent=state.track?.artist||'Pick a song in YoLite';if(state.track?.thumbnail)art.src=state.track.thumbnail;else art.removeAttribute('src');if(state.track?.id){download.hidden=false;download.href=`/download/${token}/${state.track.id}`}else download.hidden=true}catch{}}update();setInterval(update,1500);
</script></body></html>"#
        .replace("__TOKEN__", token)
}

pub(crate) fn safe_download_name(value: &str) -> String {
    let name = value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
        .take(100)
        .collect::<String>()
        .trim()
        .to_string();
    if name.is_empty() {
        "yolite-song".to_string()
    } else {
        name
    }
}

pub(crate) fn handle_controller_download(
    stream: &mut TcpStream,
    app: &tauri::AppHandle,
    state: &AppState,
    video_id: &str,
) {
    if !valid_video_id(video_id) {
        return http_response(stream, "400 Bad Request", "text/plain", b"Invalid video id");
    }
    let config = read_config();
    let cookie_file = TempCookieFile::new(&config.cookies).ok().flatten();
    let ytdlp = match ytdlp_path(app) {
        Ok(path) => path,
        Err(error) => {
            return http_response(stream, "502 Bad Gateway", "text/plain", error.as_bytes());
        }
    };
    let mut child = match external_command(&ytdlp)
        .args(ytdlp_stream_args(
            video_id,
            cookie_file.as_ref().map(|file| file.path.as_path()),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return http_response(
                stream,
                "502 Bad Gateway",
                "text/plain",
                error.to_string().as_bytes(),
            );
        }
    };
    let title = state
        .remote_state
        .lock()
        .ok()
        .and_then(|remote| remote.track.clone())
        .filter(|track| track.id == video_id)
        .map(|track| safe_download_name(&track.title))
        .unwrap_or_else(|| "yolite-song".to_string());
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: audio/mp4\r\ncontent-disposition: attachment; filename=\"{title}.m4a\"\r\ncache-control: no-store\r\nconnection: close\r\n\r\n"
    );
    if let Some(mut output) = child.stdout.take() {
        let _ = std::io::copy(&mut output, stream);
    }
    let _ = child.wait();
}

pub(crate) fn handle_controller_request(
    mut stream: TcpStream,
    app: tauri::AppHandle,
    state: Arc<AppState>,
    token: String,
) {
    let Some(request) = read_http_request(&mut stream) else {
        return;
    };
    let Some(path) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .map(|path| path.split('?').next().unwrap_or(path))
    else {
        return;
    };
    if path == format!("/controller/{token}") {
        return http_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            controller_html(&token).as_bytes(),
        );
    }
    if path == format!("/api/state/{token}") {
        let remote = state
            .remote_state
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        let body = serde_json::to_vec(&remote).unwrap_or_else(|_| b"{}".to_vec());
        return http_response(&mut stream, "200 OK", "application/json", &body);
    }
    if let Some(action) = path.strip_prefix(&format!("/api/control/{token}/")) {
        if matches!(
            action,
            "playPause" | "next" | "previous" | "volumeUp" | "volumeDown"
        ) {
            let _ = app.emit("remote-control", action.to_string());
            return http_response(&mut stream, "204 No Content", "text/plain", b"");
        }
    }
    if let Some(video_id) = path.strip_prefix(&format!("/download/{token}/")) {
        return handle_controller_download(&mut stream, &app, &state, video_id);
    }
    http_response(&mut stream, "404 Not Found", "text/plain", b"Not found");
}

pub(crate) fn start_controller_server(
    app: tauri::AppHandle,
    state: Arc<AppState>,
) -> Result<String, String> {
    let listener = TcpListener::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let token = secure_token()?;
    let url = format!("http://{}:{port}/controller/{token}", local_network_ip()?);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            let state = state.clone();
            let token = token.clone();
            thread::spawn(move || handle_controller_request(stream, app, state, token));
        }
    });
    Ok(url)
}

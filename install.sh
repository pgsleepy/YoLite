#!/bin/sh
set -eu

repository="pgsleepy/YoLite"
release_api="https://api.github.com/repos/${repository}/releases/latest"
icon_url="https://raw.githubusercontent.com/${repository}/main/src-tauri/icons/icon.png"

fail() {
  printf 'Yolite installer: %s\n' "$1" >&2
  exit 1
}

for command_name in curl sha256sum awk sed install mktemp; do
  command -v "$command_name" >/dev/null 2>&1 || fail "missing required command: ${command_name}"
done

case "$(uname -m)" in
  x86_64|amd64) ;;
  *) fail "only x86_64 Linux is currently supported" ;;
esac

temporary_directory="$(mktemp -d)"
trap 'rm -rf -- "$temporary_directory"' EXIT HUP INT TERM
release_json="${temporary_directory}/release.json"
appimage_download="${temporary_directory}/Yolite.AppImage"
icon_download="${temporary_directory}/yolite.png"

printf 'Finding the latest Yolite release…\n'
curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  "$release_api" --output "$release_json"

asset_details="$(awk '
  /"name": "Yolite_[^"]*_amd64\.AppImage",/ { appimage = 1 }
  appimage && /"digest": "sha256:/ {
    digest = $0
    sub(/^.*sha256:/, "", digest)
    sub(/".*/, "", digest)
  }
  appimage && /"browser_download_url":/ {
    url = $0
    sub(/^.*"browser_download_url": "/, "", url)
    sub(/".*/, "", url)
    print digest
    print url
    exit
  }
' "$release_json")"

expected_sha256="$(printf '%s\n' "$asset_details" | sed -n '1p')"
appimage_url="$(printf '%s\n' "$asset_details" | sed -n '2p')"
[ -n "$expected_sha256" ] || fail "release does not provide an AppImage digest"
[ -n "$appimage_url" ] || fail "release does not provide an x86_64 AppImage"

printf 'Downloading Yolite…\n'
curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location --retry 3 \
  "$appimage_url" --output "$appimage_download"
actual_sha256="$(sha256sum "$appimage_download" | awk '{print $1}')"
[ "$actual_sha256" = "$expected_sha256" ] || fail "download checksum did not match GitHub"

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  "$icon_url" --output "$icon_download"

install_home="${YOLITE_INSTALL_HOME:-${HOME}}"
data_home="${YOLITE_DATA_HOME:-${XDG_DATA_HOME:-${install_home}/.local/share}}"
bin_directory="${install_home}/.local/bin"
install_directory="${install_home}/.local/opt/yolite"
appimage_path="${install_directory}/Yolite.AppImage"
desktop_path="${data_home}/applications/dev.yolite.client.desktop"
icon_path="${data_home}/icons/hicolor/512x512/apps/yolite.png"

desktop_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g'
}

install -d "$bin_directory" "$install_directory" "$(dirname "$desktop_path")" "$(dirname "$icon_path")"
install -m 755 "$appimage_download" "${appimage_path}.new"
mv -f "${appimage_path}.new" "$appimage_path"
install -m 644 "$icon_download" "$icon_path"

escaped_appimage_path="$(desktop_escape "$appimage_path")"
launcher_temporary="${temporary_directory}/yolite"
{
  printf '%s\n' '#!/bin/sh'
  printf 'exec "%s" "$@"\n' "$escaped_appimage_path"
} > "$launcher_temporary"
install -m 755 "$launcher_temporary" "${bin_directory}/yolite"

desktop_temporary="${temporary_directory}/dev.yolite.client.desktop"
{
  printf '%s\n' '[Desktop Entry]'
  printf '%s\n' 'Name=Yolite'
  printf '%s\n' 'Comment=Lightweight YouTube Music client'
  printf 'Exec="%s"\n' "$escaped_appimage_path"
  printf '%s\n' 'Icon=yolite'
  printf '%s\n' 'Terminal=false'
  printf '%s\n' 'Type=Application'
  printf '%s\n' 'Categories=AudioVideo;Audio;Player;'
  printf '%s\n' 'StartupWMClass=Yolite'
} > "$desktop_temporary"
install -m 644 "$desktop_temporary" "$desktop_path"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$(dirname "$desktop_path")" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --ignore-theme-index "${data_home}/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
  kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
  kbuildsycoca5 --noincremental >/dev/null 2>&1 || true
fi

printf '\nYolite installed successfully.\n'
printf 'Launch it from your application menu or run: %s\n' "${bin_directory}/yolite"
case ":${PATH}:" in
  *":${bin_directory}:"*) ;;
  *) printf 'Add %s to PATH if you want to run just: yolite\n' "$bin_directory" ;;
esac
if ! command -v fusermount >/dev/null 2>&1 && ! command -v fusermount3 >/dev/null 2>&1; then
  printf 'AppImage note: install FUSE if Yolite does not start (Arch/CachyOS: sudo pacman -S fuse2).\n'
fi

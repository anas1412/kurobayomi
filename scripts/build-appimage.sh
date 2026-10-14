#!/usr/bin/env bash
# Packs a built kurobayomi binary into an AppImage: GTK 4 (through linuxdeploy's GTK plugin), libmpv and the other shared
# libraries travel inside it.
#
# Usage: [CHANNEL=nightly] scripts/build-appimage.sh BINARY OUTPUT.AppImage
# CHANNEL=nightly packs the Nightly app: its own app id, name and gold icon, and it leaves the file types and links to the
# stable app. Without it the stable app is packed.
# Needs the libraries the binary links (libgtk-4, libmpv, ...) installed on this machine: they are what gets bundled, so
# build on the oldest distro you want the AppImage to run on (CI uses Ubuntu 24.04, the first with GTK 4.14).
set -euo pipefail

bin="$(readlink -f "${1:?usage: build-appimage.sh BINARY OUTPUT.AppImage}")"
out="$(readlink -m "${2:?usage: build-appimage.sh BINARY OUTPUT.AppImage}")"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data="$root/src/shell/data"
id=io.github.anas1412.Kurobayomi
icons="$data/icons"
if [ "${CHANNEL:-stable}" = nightly ]; then
    id=$id.Nightly
    icons="$data/icons-nightly"
fi
tools="${TOOLS_DIR:-$root/target/appimage-tools}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
appdir="$work/AppDir"

# the packaging tools, pinned by version and checked by hash
fetch() { # fetch NAME URL SHA256
    local file="$tools/$1"
    if ! { [ -f "$file" ] && echo "$3  $file" | sha256sum -c --status; }; then
        mkdir -p "$tools"
        curl -fsSL "$2" -o "$file"
        echo "$3  $file" | sha256sum -c --status || { echo "checksum mismatch: $1" >&2; exit 1; }
    fi
    chmod +x "$file"
}
fetch linuxdeploy https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
fetch linuxdeploy-plugin-gtk.sh https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/7a3fbc31a9e5075073ff8790f26effbac5f84453/linuxdeploy-plugin-gtk.sh b0f4cbc684a0103a9651f0955b635eaea0096b3a66c0f5a2c2aa337960375171
fetch appimagetool https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0

# the app's files, laid out like an install into /usr
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/mime/packages"
install -m755 "$bin" "$appdir/usr/bin/kurobayomi"
cp -r "$icons" "$appdir/usr/share/"
# no D-Bus activation from inside an AppImage: there is no installed service file for the bus to start
desktop="$data/io.github.anas1412.Kurobayomi.desktop"
if [ "${CHANNEL:-stable}" = nightly ]; then
    sed -e "s/io\.github\.anas1412\.Kurobayomi/$id/g" -e 's/^Name=Kurobayomi$/Name=Kurobayomi Nightly/' \
        -e '/^DBusActivatable=/d' -e '/^MimeType=/d' "$desktop" > "$appdir/usr/share/applications/$id.desktop"
else
    install -m644 "$data/$id.xml" "$appdir/usr/share/mime/packages/$id.xml"
    grep -v '^DBusActivatable=' "$desktop" > "$appdir/usr/share/applications/$id.desktop"
fi

# the plugin must be on PATH under the name linuxdeploy looks for; the AppImages run unpacked so no FUSE is needed
mkdir -p "$work/bin"
ln -s "$tools/linuxdeploy-plugin-gtk.sh" "$work/bin/linuxdeploy-plugin-gtk.sh"
export PATH="$work/bin:$PATH" APPIMAGE_EXTRACT_AND_RUN=1 DEPLOY_GTK_VERSION=4 NO_STRIP=1
# NO_STRIP keeps file and line numbers in the app's crash reports

"$tools/linuxdeploy" --appdir "$appdir" --plugin gtk \
    --executable "$appdir/usr/bin/kurobayomi" \
    --desktop-file "$appdir/usr/share/applications/$id.desktop" \
    --icon-file "$icons/hicolor/256x256/apps/$id.png" --icon-filename "$id"

# GTK's gstreamer media module links the host's gstreamer, which fails against the bundled glib on newer distros (an
# "undefined symbol" warning at start); the app plays video through libmpv, so the module is not needed
rm -rf "$appdir"/usr/lib/gtk-4.0/*/media

mkdir -p "$(dirname "$out")"
# update information for AppImage updaters (Gear Lever, AppImageUpdate, ...): they fetch only the changed blocks of the
# newest nightly file. appimagetool runs zsyncmake, which writes the .zsync file into the current folder, so run it in the
# output folder to get it next to the AppImage.
(cd "$(dirname "$out")" && ARCH=x86_64 "$tools/appimagetool" --no-appstream \
    -u "gh-releases-zsync|anas1412|kurobayomi|nightly|Kurobayomi-nightly-x86_64.AppImage.zsync" "$appdir" "$(basename "$out")")
[ -f "$out.zsync" ] || { echo "no $out.zsync was written (is zsyncmake installed?)" >&2; exit 1; }
echo "built $out ($(du -h "$out" | cut -f1)) and $out.zsync"

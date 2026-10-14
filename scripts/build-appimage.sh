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
# what an AppImage updater (Gear Lever, AppImageUpdate, ...) follows: the nightly release, or the newest stable one
update_info="gh-releases-zsync|anas1412|kurobayomi|latest|Kurobayomi-x86_64.AppImage.zsync"
if [ "${CHANNEL:-stable}" = nightly ]; then
    id=$id.Nightly
    icons="$data/icons-nightly"
    update_info="gh-releases-zsync|anas1412|kurobayomi|nightly|Kurobayomi-nightly-x86_64.AppImage.zsync"
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

# The GTK plugin's launcher hook forces `GTK_THEME=Adwaita:<light|dark>` (so the app ignores the user's GTK theme) and
# `GDK_BACKEND=x11` (so it runs through XWayland on a Wayland desktop). Both are removed: the app follows the user's theme
# and uses the session's own display protocol. The check fails the build if a plugin update brings either line back.
hook="$appdir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
sed -i -e '/^COLOR_SCHEME=/,/^APPIMAGE_GTK_THEME=/d' -e '/^export GTK_THEME=/d' -e '/^export GDK_BACKEND=/d' "$hook"
if grep -qE '^export (GTK_THEME|GDK_BACKEND)=|APPIMAGE_GTK_THEME' "$hook"; then
    echo "the AppRun hook still forces a theme or a display backend: $hook" >&2
    exit 1
fi

# GTK's gstreamer media module links the host's gstreamer, which fails against the bundled glib on newer distros (an
# "undefined symbol" warning at start); the app plays video through libmpv, so the module is not needed
rm -rf "$appdir"/usr/lib/gtk-4.0/*/media

# The licences that travel with the AppImage: ours (LICENSE, NOTICE) and, for every bundled system library, the copyright file
# of the Debian/Ubuntu package it came from, with that package's version, so the source of each library can be found
# (SOURCE-OFFER.txt). Without dpkg (a local build on another distro) the library part is skipped with a warning; on the CI
# runner it must find something, or the build fails.
lic="$appdir/usr/share/licenses/kurobayomi"
mkdir -p "$lic/libraries"
install -m644 "$root/LICENSE" "$root/NOTICE" "$lic/"
cp "$data/SOURCE-OFFER.txt" "$lic/SOURCE-OFFER.txt"
if command -v dpkg >/dev/null; then
    : > "$lic/libraries/PACKAGES.txt"
    for lib in "$appdir"/usr/lib/*.so*; do
        found="$(ldconfig -p | awk -v n="$(basename "$lib")" '$1 == n {print $NF; exit}')"
        if [ -n "$found" ]; then
            dpkg -S "$(readlink -f "$found")" 2>/dev/null | head -1 | cut -d: -f1 || true
        fi
    done | sort -u | while read -r pkg; do
        [ -f "/usr/share/doc/$pkg/copyright" ] && cp "/usr/share/doc/$pkg/copyright" "$lic/libraries/$pkg.copyright"
        dpkg-query -W -f='${Package} ${Version}\n' "$pkg" >> "$lic/libraries/PACKAGES.txt"
    done
    [ -s "$lic/libraries/PACKAGES.txt" ] || { echo "no bundled library could be traced to a package: the licences are missing" >&2; exit 1; }
else
    echo "warning: dpkg not found, the licence texts of the bundled libraries are not included" >&2
fi

mkdir -p "$(dirname "$out")"
# update information for AppImage updaters (Gear Lever, AppImageUpdate, ...): they fetch only the changed blocks of the
# newest file of the channel. appimagetool runs zsyncmake, which writes the .zsync file into the current folder, so run it
# in the output folder to get it next to the AppImage.
(cd "$(dirname "$out")" && ARCH=x86_64 "$tools/appimagetool" --no-appstream -u "$update_info" "$appdir" "$(basename "$out")")
[ -f "$out.zsync" ] || { echo "no $out.zsync was written (is zsyncmake installed?)" >&2; exit 1; }
echo "built $out ($(du -h "$out" | cut -f1)) and $out.zsync"

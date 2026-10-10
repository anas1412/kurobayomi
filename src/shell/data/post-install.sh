#!/bin/sh
# Package post-install hook (T-53a-041): refresh the desktop, MIME and icon caches.
set -e
update-desktop-database -q /usr/share/applications || true
update-mime-database /usr/share/mime || true
gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true

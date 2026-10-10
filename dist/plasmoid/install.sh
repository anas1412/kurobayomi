#!/usr/bin/env bash
# Installs or removes the two Plasma applets for the current user with kpackagetool6.
#   dist/plasmoid/install.sh install   (also the default)
#   dist/plasmoid/install.sh remove
# The applets are optional: the app runs without them.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
action="${1:-install}"
for id in org.kurobayomi.updates.manga org.kurobayomi.updates.anime; do
    case "$action" in
        install)
            # --upgrade when the applet is already installed, a plain install otherwise
            kpackagetool6 --type Plasma/Applet --upgrade "$id" 2>/dev/null || kpackagetool6 --type Plasma/Applet --install "$id"
            ;;
        remove) kpackagetool6 --type Plasma/Applet --remove "$id" ;;
        *) echo "usage: $0 [install|remove]" >&2; exit 2 ;;
    esac
done

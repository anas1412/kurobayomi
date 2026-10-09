# Topic: TorrServer settings page

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Owner area: 48-settings-player-and-scripts. The server itself (TorrServer, the Go streaming server bundled through JNI) and its HTTP API are owned by area:13-core-util-storage-archive-torrent. This topic covers the settings page, its preference keys, validation, and the service lifecycle as seen from the settings.

## 1. Preferences (`core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt`)

| Accessor | Key | Type | Default | Notes |
|---|---|---|---|---|
| `torrServerEnable()` | `pref_torrserver_enable` | Boolean | `false` | |
| `torrServerShownNotice()` | `pref_torrserver_shownotice` | Boolean | `false` | set to `true` the first time the switch is turned on |
| `torrServerPort()` | `pref_torrserver_port` | String | `"8090"` | stored as text, validated as an integer |
| `torrServerTrackers()` | `pref_torrserver_tackers` | String | 26-line list (section 3) | key spelling `tackers` is intentional in the source (typo preserved for compatibility) |
| `torrServerProxyMode()` | `pref_torrserver_proxymode` | enum `ProxyMode` | `None` | stored by constant name |
| `torrServerProxyUrl()` | `pref_torrserver_proxyurl` | String | `""` | |

`ProxyMode` (`TorrentPreferences.kt:46-51`): `None("tracker")`, `Tracker("tracker")`, `Peers("peers")`, `Full("full")`. The `value` string is what the server receives.

**OBSERVED QUIRK:** `None` and `Tracker` both have the value `"tracker"`. `TorrentServerService.startServer` (`app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt:70-75`) passes `proxyMode.value` and passes an empty proxy URL when the mode is `None`. So the server receives mode `"tracker"` with an empty URL when the user chose "None". Whether the server treats an empty URL as "no proxy" is outside this area (unverified). The Rust port must not reuse the duplicate value; map `None` to "no proxy" explicitly and send the proxy mode only when a proxy URL is set (proposal, see section 6).

## 2. Screen (`app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsTorrentScreen.kt`)

Title: `pref_player_torrents` ("Torrent"). Items in order:

1. **Enable TorrServer** switch (`pref_player_torrents_enable`, "Enable TorrServer").
   - Turning on for the first time (`it == true && !shownNotice`) shows the Legal Notice dialog and sets `shownNotice = true` (the set happens before the dialog closes). Subsequent enables show nothing.
   - Turning off calls `TorrentServerService.stop()`.
   - Returns `true` in all cases (value always saved).
2. **TorrServer port** (`pref_player_torrents_port`, "TorrServer port"). Dialog subtitle `pref_player_torrents_port_summary` ("Enter 0 to let system choose a port"). Keyboard: number. Enabled iff the switch is on. Validation: `pref.toIntOrNull()` must be non-null and in `0..65535`; otherwise invalid, error text `pref_player_torrents_port_error` ("Port must be a number between 0 and 65535"). Empty string is invalid (no `canBeBlank`). No `onValueChanged`, so changing the port does **not** restart a running server (observed; the new port applies at the next start).
3. **Torrent trackers** (`pref_player_torrents_trackers`, "Torrent trackers"). Multi-line dialog. Subtitle: first two lines joined by `\n`, plus `\n...` when there are more than two lines (computed from the current value). Enabled iff the switch is on. `onValueChanged` calls `TorrentServerService.stop()` (the server must be restarted to use the new list; the stop is not followed by a restart, so the user must turn the switch on again).
4. **Reset default torrent trackers string** (`pref_player_torrents_trackers_reset`, "Reset default torrent trackers string"). Text row. Enabled iff `enable && trackers != default`. Click deletes the preference (value returns to default).
5. **Proxy mode** (`pref_player_torrents_proxy_mode`, "Proxy mode"). List preference with entries: `None` → `pref_player_torrents_proxy_mode_none` "None"; `Tracker` → `..._tracker` "Tracker"; `Peers` → `..._peers` "Peers"; `Full` → `..._full` "Full". Enabled iff the switch is on.
6. **Proxy url** (`pref_player_torrents_proxy_url`, "Proxy url"). Dialog subtitle `pref_player_torrents_proxy_url_dialog` ("Supports http, https, socks4, socks4a, socks5, socks5h"). Enabled iff `enable && proxyMode != None`.
   - `validate(url)`: parse with `Uri.parse`-equivalent; invalid if `scheme == null || host == null`; invalid if `scheme` not in `{http, https, socks4, socks4a, socks5, socks5h}`.
   - Error text: missing scheme or host → `pref_player_torrents_proxy_url_invalid_uri` ("Invalid uri"); bad scheme → `pref_player_torrents_proxy_url_invalid_protocol` ("Unsupported protocol: \"%s\"") with the scheme.
   - **OBSERVED: the empty string is invalid**, so OK is disabled for an empty value and the proxy URL cannot be cleared through this dialog once set. The only way to clear it is to reset the database value (not offered in UI). Edge E-TS-03.

Dialog text (legal notice): title `pref_player_torrents_notice` ("Legal Notice"); body paragraph 1 `pref_player_torrents_notice_text` = "TorrServer relies on peer to peer file sharing to access content. By doing so, you also upload files to others. Any content you share is your sole responsibility."; paragraph 2 `pref_player_torrents_notice_footer` = "No further notices will be issued."; OK `action_ok`.

## 3. Default trackers string (exact)

`TorrentPreferences.kt`: a raw string whose lines are indented, then `.replace(" ", "")`. Result: 26 lines joined by `\n`, no leading or trailing whitespace on any line, no trailing newline.

```
http://nyaa.tracker.wf:7777/announce
http://anidex.moe:6969/announce
http://tracker.anirena.com:80/announce
udp://tracker.uw0.xyz:6969/announce
http://share.camoe.cn:8080/announce
http://t.nyaatracker.com:80/announce
udp://47.ip-51-68-199.eu:6969/announce
udp://9.rarbg.me:2940
udp://9.rarbg.to:2820
udp://exodus.desync.com:6969/announce
udp://explodie.org:6969/announce
udp://ipv4.tracker.harry.lu:80/announce
udp://open.stealth.si:80/announce
udp://opentor.org:2710/announce
udp://opentracker.i2p.rocks:6969/announce
udp://retracker.lanta-net.ru:2710/announce
udp://tracker.cyberia.is:6969/announce
udp://tracker.dler.org:6969/announce
udp://tracker.ds.is:6969/announce
udp://tracker.internetwarriors.net:1337
udp://tracker.openbittorrent.com:6969/announce
udp://tracker.opentrackr.org:1337/announce
udp://tracker.tiny-vps.com:6969/announce
udp://tracker.torrent.eu.org:451/announce
udp://valakas.rollo.dnsabr.com:2710/announce
udp://www.torrent.eu.org:451/announce
```

Reset: deleting the preference returns `get()` to this value. Trackers are passed to the server by `TorrentServerUtils.setTrackersList()` (area:13) about 10 time units after the server starts (`TorrentServerService.kt`: `wait(10)` then `setTrackersList()`; the unit of `wait` is not verified here).

## 4. Service lifecycle as seen from settings

- `TorrentServerService` (`app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt`):
  - `startServer()` runs on a service-scoped coroutine. If `api.echo()` returns an empty string (no server answering), it starts the server: `TorrServer.startServer(port = torrServerPort, path = filesDir.absolutePath, proxyMode = proxyMode.value, proxyUrl = (None ? "" : torrServerProxyUrl))`. A return value of `-1` means failure. On success: `api.setPort(port)`, wait, `setTrackersList()`.
  - `stopServer()`: `TorrServer.stopServer()`, cancel notification `Notifications.ID_TORRENT_SERVER`, `stopSelf()`.
  - `START_NOT_STICKY` is returned from `onStartCommand`.
- Settings call `TorrentServerService.stop()` on switch-off and on trackers change. Other entry points (player, downloader, anime screen) read `torrServerEnable()` and start the service on demand (area:45 and area:24).

## 5. Port decisions (Linux)

- TorrServer is a Go program. On Linux, do not embed it. **Replace-with-process**: the app runs a configured `torrserver` executable (path in preferences, default: search `PATH`), with the port, data directory (`$XDG_DATA_HOME/kurobayomi/torrserver`), and the trackers list supplied as its arguments or config file, as the upstream binary requires. Verify the upstream CLI flags before implementation (unverified in this area).
- Keep the settings page as described, with these changes: the port change restarts a running server; the trackers change restarts the server automatically (instead of leaving it stopped); the proxy URL may be cleared.
- Stop the child process on app exit and on switch-off. Do not leave orphan processes (check `prctl` / process group on exit).
- Legal notice: show once, keep the text.

## 6. Proposed validation rules (Rust)

- Port: parse as `u16`; accept `0..=65535`; empty string is invalid. Message: "Port must be a number between 0 and 65535".
- Proxy URL: empty is valid when the mode is `None`; otherwise parse with the `url` crate. Require host; scheme in `{http, https, socks4, socks4a, socks5, socks5h}`. Messages as in section 2.
- Proxy mode `None`: send no proxy arguments at all (do not send "tracker"). Mode `Tracker`/`Peers`/`Full` pass their value strings `tracker` / `peers` / `full` (these are the upstream values, unverified).

## 7. Edge cases

See `p1-edge-cases-and-tests.md`, group E-TS.

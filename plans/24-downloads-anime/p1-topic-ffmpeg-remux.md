# Topic: ffmpeg remux pipeline (anime downloads)

Deep dive for `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt` lines 568-701 (`ffmpegDownload`, `getFFmpegOptions`, `getDuration`). Authoritative rules are in `p1-behavior.md` §5.7. This note shows one worked example so an implementer can check the Rust argv builder by eye.

## 1. Why remux, not re-encode

Every stream is copied (`-c:v copy -c:a copy -c:s copy`). The output is always Matroska (`-f matroska`). The download is therefore I/O-bound, and quality is whatever the source delivers. Consequence: no CPU budget is needed beyond the demuxer; several downloads can run in parallel (up to three sources, `AnimeDownloader.kt:212`).

## 2. Inputs for the worked example

A `Video` as the source returns it (fields from `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:30-46`):

| Field | Value |
|---|---|
| `videoUrl` | `https://cdn.example/v.m3u8` |
| `headers` | `[("Referer", "https://example.org/"), ("User-Agent", "UA/1.0")]` |
| `ffmpegStreamArgs` | `[("allowed_extensions", "ALL")]` |
| `subtitleTracks` | `[Track(url = "https://cdn.example/en.vtt", lang = "English")]` |
| `audioTracks` | `[Track(url = "https://cdn.example/ja.m3u8", lang = "Japanese")]` |
| `ffmpegVideoArgs` | `[]` |
| output | `/data/dl/Example (EN)/Show/Scan_Episode 1_tmp/Episode 1.tmp` |

The source has no per-video headers beyond the list above, so the same header string is used for all three inputs (they are all `http`).

## 3. The header string

`headers.joinToString("", "-headers '", "'") { "${name}: ${value}\r\n" }` gives the string below, where `\r\n` are real CR LF characters:

```
Referer: https://example.org/\r\nUser-Agent: UA/1.0\r\n
```

In Rust the same value is one argv element, with no quotes around it. CR and LF inside values are removed first (behaviour §E-F13, T-24a-070).

## 4. Argv in Kotlin order (what the Rust builder must produce)

Shown one element per line. The Rust builder adds `-progress pipe:1 -nostats -stats_period 0.5` at the very front (design §7), which the Kotlin build does not have. Everything else is the same order as `getFFmpegOptions`.

```
-progress
pipe:1
-nostats
-stats_period
0.5
-headers
Referer: https://example.org/\r\nUser-Agent: UA/1.0\r\n
-allowed_extensions
ALL
-i
https://cdn.example/v.m3u8
-headers
Referer: https://example.org/\r\nUser-Agent: UA/1.0\r\n
-i
https://cdn.example/en.vtt
-headers
Referer: https://example.org/\r\nUser-Agent: UA/1.0\r\n
-i
https://cdn.example/ja.m3u8
-map
0:v
-map
2:a
-map
0:a?
-map
1:s
-map
0:s?
-map
0:t?
-f
matroska
-c:a
copy
-c:v
copy
-c:s
copy
-metadata:s:s:0
title=English
-metadata:s:a:0
title=Japanese
/data/dl/Example (EN)/Show/Scan_Episode 1_tmp/Episode 1.tmp
-y
```

Index arithmetic (behaviour §5.7.1):
- Input 0 is the video.
- Subtitle inputs start at index 1 (`formatMaps(..., "s", offset = 0)` gives `i + 1`): the English track is `1:s`.
- Audio inputs start after subtitle inputs (`offset = subtitleTracks.size = 1`): the Japanese track is `i + 1 + 1 = 2:a`.
- `-map 0:a?` and `-map 0:s?` take the main input's optional streams, and `-map 0:t?` takes attachments.

Note the `-headers` argument appears before every `http` input. Had the subtitle URL been a local file, its `-headers` pair would be omitted.

## 5. Expected output

With a successful remux the file `Episode 1.tmp` is renamed to `Episode 1.mkv` (`AnimeDownloader.kt:617-619`). The final container has:
- stream 0: the video (copied from the HLS source);
- one audio stream from the Japanese input, tagged `title=Japanese`;
- one subtitle stream from the English input, tagged `title=English`;
- any embedded audio or subtitle streams from the main input, added after the mapped ones.

Stream order is therefore: video, mapped audio, embedded audio, mapped subtitles, embedded subtitles, attachments. Players select tracks by language title; the order is not significant to Kurobayomi's player, but it is significant for tests that compare stream indices. Keep the same order to match Aniyomi output.

## 6. ffprobe duration call

Command (behaviour §5.7):

```
ffprobe -headers "Referer: https://example.org/\r\nUser-Agent: UA/1.0\r\n" -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 https://cdn.example/v.m3u8
```

Important details:
- The header prefix is included even when the video URL is a local file (`headerOptions` is never empty, `AnimeDownloader.kt:578-581`). ffprobe ignores header options for file inputs, so this is harmless. The Rust port may omit it for local files.
- The output is one float line such as `1421.504000`. `toFloatOrNull()` is used, so `N/A` gives null, which means duration 0 and no progress.
- Only the duration is needed. The ffprobe call happens once before the remux.

## 7. Progress parsing

ffmpeg writes `key=value` lines to stdout with `-progress pipe:1`. The Rust reader consumes these:

```
frame=120
fps=25.00
out_time_us=4800000
bitrate=1523.1kbits/s
speed=1.02x
progress=continue
```

Rules:
- `out_time_us` is microseconds. The Kotlin `StatisticsCallback.time` is milliseconds. Do not copy the Kotlin `/1000.0` blindly; use `/1_000_000`.
- `progress=end` marks completion. Exit status is then checked (success = 0).
- Percent = `min(100, 100 * floor(out_seconds) / duration_seconds)` with integer math. This matches `(100 * outTime / duration).toInt()` (`AnimeDownloader.kt:606`) because `outTime` is truncated to whole seconds there.
- Publish only when the integer changes, at most every 100 ms.

## 8. Failure and cancel

| Event | Result | Partial output |
|---|---|---|
| ffmpeg exit != 0 | `Error in ffmpeg!`, retry (behaviour §5.5) | deleted |
| ffmpeg exit 0 but `.tmp` size 0 | error (proposal, T-24a-067) | deleted |
| ffprobe exit != 0 | exception with ffprobe output, retry | none yet |
| Pause or cancel | child killed; `wait()` awaited; `.tmp` deleted | deleted |
| Remux ends, rename fails | error (`isDownloadSuccessful` path, fixed in T-24a-049) | kept |

Kill semantics in Rust (design §14): `Child::start_kill()`, then `wait()`, then delete the file. Deleting a file that ffmpeg still holds open is allowed on Linux, but the write must be finished first to avoid a half-written file reappearing.

## 9. Test checklist

- Golden argv for: video only; video + 1 subtitle; video + 1 audio; video + both; subtitle local file (no header); `ffmpeg_stream_args` empty and non-empty; CRLF inside a header value.
- Progress parser with `out_time_us` lines, `progress=end`, and garbage lines.
- A fake `ffmpeg` script that writes its argv to a log file and emits progress lines; use it to assert argv order end to end.
- A fake `ffprobe` that prints `N/A` and a number.

## 10. Packaging note

The binary is `ffmpeg` and `ffprobe` on `PATH`. On CachyOS / Arch these come from the `ffmpeg` package (which provides both). Startup check per T-24a-067. Do not bundle ffmpeg; the system build already carries the codecs and HLS/TLS support the sources need.

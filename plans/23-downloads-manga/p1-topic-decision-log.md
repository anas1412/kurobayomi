# Area 23 — Decision log (open questions for the user)

Each item needs a yes or no from the user before the matching task is started. The recommendation is what the plan assumes until the user answers. Items reference the defect register (`p1-topic-defect-register.md`) and tasks (`p1-tasks.md`).

| ID | Question | Options | Recommendation (assumed) | Tasks |
|---|---|---|---|---|
| D-DL-01 | Use Windows-safe folder names (reserved device names, trailing dots) on Linux too? | (a) Kotlin FAT rules only; (b) also reserve CON, PRN, AUX, NUL, COM1-9, LPT1-9 | (b). Copied folders stay readable on the Windows port (addendum rule 5); only titles with reserved names differ | T-23a-003 |
| D-DL-02 | The Kotlin "Maximum downloads" setting (`download_slots`) does nothing for manga. What should the Rust control do? | (a) Keep as "Maximum downloads", default 5, cap 5, renamed `downloads.max_parallel_chapters`; (b) remove the control | (a). It gives the user a real knob for bandwidth | T-23a-037 |
| D-DL-03 | Importance of failure notifications | (a) LOW (Kotlin: silent); (b) DEFAULT (sound and banner) | (b) for errors and warnings, LOW for progress and paused. A desktop user needs to notice a failed chapter | T-23a-019 |
| D-DL-04 | What does "Download only over Wi-Fi" mean on Linux? | (a) not on a metered connection (GIO `network_metered`); (b) NetworkManager device type "wifi" over D-Bus | (a). GIO is portable and needs no extra dependency. The label in the settings must say "metered" on Linux | T-23a-023 |
| D-DL-05 | "Move to top" in the queue | (a) moves within its source group only; (b) Kotlin rebuild, which can reorder other sources too | (a). Keeps the sources interleaving the user already sees | T-23a-025 |
| D-DL-06 | After the network returns, resume automatically? | (a) yes, unless the user paused; (b) never (Kotlin) | (a), with network-only errors returned to QUEUE | T-23a-023 |
| D-DL-07 | Keep downloads running when the window is closed? | (a) yes, process kept alive with GApplication hold while work exists; (b) stop when the window closes (Kotlin has no window concept) | (a). It matches the Android background behaviour that users rely on | T-23a-023, T-23a-011 |
| D-DL-08 | Two chapters map to the same folder name | (a) silently treat the second as downloaded (Kotlin); (b) warn at enqueue | (b). Silent skipping hides real chapters | T-23a-024 |
| D-DL-09 | resmush.it data-saver service | (a) keep, over HTTPS with `serde_json`; (b) drop | (b). It is a plain-HTTP third-party service and the Kotlin parses JSON by string search. Final call belongs to area:26 | T-23a-017 |
| D-DL-10 | Write `.nomedia` at the downloads root | (a) never (Kotlin effectively); (b) once at the root | (a). Desktop gallery indexers are not a target. Revisit if the user asks | T-23a-022 |
| D-DL-11 | Retry HTTP 404 and 410 | (a) retry like every other error (Kotlin); (b) fail at once | (b). Retrying a missing page wastes 14 seconds per page | T-23a-013 |
| D-DL-12 | Image format support for AVIF, HEIF and JXL in the Rust build | (a) recognise and store; reader decodes via area:43; (b) reject | (a). Mirrors Kotlin `ImageType`. The split step skips them | T-23a-014, T-23a-018 |
| D-DL-13 | Split tall images above 256 MiB decoded | (a) skip the split with a log line; (b) band-wise decode | (a) for the first release; (b) later if the user needs it | T-23a-018 |
| D-DL-14 | Pending deletes flushed on reader exit only (Kotlin), or also at startup | (a) reader exit only; (b) also at startup | (b). A crash on reader exit would otherwise keep read chapters forever | T-23a-028 |

## Notes on what is not an open question

- The download folder layout, file names, CBZ format and ComicInfo fields are fixed by compatibility with Aniyomi downloads and are not open.
- Retry schedule, concurrency (5 sources, 2 pages), 200 MiB minimum and warning thresholds are kept (`p1-topic-defect-register.md` section "Keep").
- The Android-only machinery drop (T-23a-038) and the state-migration drop (T-23a-039) are final unless the user asks for a Windows-first port of the queue file.

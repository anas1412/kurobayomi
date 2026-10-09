# MyAnimeList live verification checklist (T-29a-058)

Run against a test MyAnimeList account before the MAL tracker is marked done. Record each result in the
Result column with the date and the observed value. Until a row is filled in, the decision it closes stays open.
No automated test in this crate contacts myanimelist.net.

Setup: register an application at https://myanimelist.net/apiconfig, put its client id in
`tracking/myanimelist.toml` as `client_id` (or set `KUROBAYOMI_MAL_CLIENT_ID`), and register the redirect URI
`http://localhost:8765/myanimelist-auth`.

| # | Check | Decision it closes | Expected | Result |
|---|-------|--------------------|----------|--------|
| 1 | Does MAL accept `http://localhost:8765/myanimelist-auth` as the redirect URI? | T-29a-039, custom scheme fallback | Login completes through the loopback listener | open |
| 2 | Duplicate `q` values in a search URL | D-09 | Only one `q` is sent by the client | open |
| 3 | Partial dates (`2020-05`, `2020`) in detail responses | D-08 | Parsed as the 1st of the month or the 1st of January | open |
| 4 | `score` sent as `8.0` versus `8` | D-11 | Which form MAL stores; set `WIRE_INTEGER_SCORE` if `8` is needed | open |
| 5 | `score=-1` on PUT | D-12 | Rejected by MAL; the client never sends it | open |
| 6 | Anime list endpoint shape (`/users/@me/animelist`) | D-01 | Keys match `dto::AnimeListStatus` | open |
| 7 | `paging` absent on the last page of a user list | E-SEARCH-16 | The walk ends without error | open |
| 8 | DELETE of a title not on the list | E-DEL-1 | HTTP 404, logged as a warning, local row removed | open |
| 9 | Token refresh response shape (`refresh_token` present, `created_at` absent) | E-AUTH-8 | `MalToken` decodes it | open |
| 10 | Rate-limit response (HTTP 429) on search or PUT | E-NET | Error text `HTTP error 429` is shown | open |
| 11 | Consent denied in the browser | E-LOGIN-1 | "Login was not completed" and the old login stays | open |
| 12 | Paste of the full redirect URL and of the bare code | E-LOGIN-9 | Both complete the login | open |
| 13 | Revoke the app on the MAL website, then make a call | E-UI-2 | "Log in again" appears on the Settings row | open |

# Topic: WASM component extension format

## Package layout

```
<data>/extensions/<pkg>/
  manifest.toml        see p1-data-and-config.md D8
  component.wasm       WASM component implementing world manga-extension or anime-extension
  component.sha256     hex sha256 of component.wasm, written at install
  settings.json        optional settings schema (key, kind, default, label key)
```

## Manifest validation (order)

1. Parse TOML. Unknown keys are ignored (forward compatibility).
2. `pkg` matches `^[a-z0-9_]+(\.[a-z0-9_]+)+$`.
3. `version_code` is an integer >= 1.
4. `api` parses as `major.minor`; major must be `1`; minor must be `0` (the only version this host implements at first release). Newer minors are rejected with `Error(UnsupportedApi)`.
5. `kind` in `manga | anime | both`.
6. `sources` non-empty; each `id` string parses to `i64`; recomputed id via `kb_source::id::source_id(name, lang, version_id)` must equal the stored id when `version_id` is provided in the entry (optional field `version_id`, default 1).
7. `allowed_hosts` (new field, list of host names) bounds the `http` import.

## Component checks

- Hash of `component.wasm` equals `component.sha256`.
- Component exports the world's functions; a missing export is `Error(MissingExport)`.
- Imports allowed are exactly `http` and `settings`. Any other import is `Error(ForbiddenImport)`.

## Host functions

| Import | Signature | Notes |
|---|---|---|
| `http.fetch` | `(request: http-request) -> result<http-response, source-error>` | `http-request { method: string, url: string, headers: list<tuple<string,string>>, body: option<list<u8>> }`; host checks `url` host against `allowed_hosts`; default timeout 30 s; body cap 16 MiB |
| `settings.get` | `(key: string) -> option<string>` | per-source namespace |
| `settings.set` | `(key: string, value: string) -> result<_, source-error>` | writes `source_settings` row |
| `clock.now-ms` | `() -> s64` | epoch milliseconds |

Host behaviour for `fetch`: the host applies the same User-Agent default as the built-in sources and the per-host rate limit from area:12-network-stack. Redirects are followed up to 10 (reqwest default). Cookies are per source, stored in the source's cookie jar (not shared between sources).

## Limits

| Limit | Value | Reason |
|---|---|---|
| Fuel per call | 10,000,000,000 (**Proposal**, to tune) | bound CPU |
| Epoch deadline | 5 s per call | wall-clock guard |
| Memory | 256 MiB | bound RAM |
| Instantiate timeout | 2 s | keeps loader responsive |
| Response body | 16 MiB | bound memory from fetch |
| Concurrent calls per extension | 4 | bound load on the host |

## Versioning

The WIT package version is `kurobayomi:source@1.0.0`. Adding a function is a minor bump, which old components still satisfy only if they do not import it. Removing or changing a signature is a major bump; the loader rejects it.

## Authoring

- A reference guest crate `kb-ext-template` (to be created under T-20a-001) with `wit-bindgen` generated bindings.
- Build: `cargo component build --release` produces `component.wasm`; `sha256sum` writes `component.sha256`.
- Testing: the host crate ships a test harness that loads the component and calls each export with fixture input.

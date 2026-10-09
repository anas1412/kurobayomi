# Area 18 — Topic: package loading, selection, and trust

Deep dive for the load pipeline and the trust decision. Full rules: `p1-behavior.md` sections 3, 10 and 16. Types: `p1-rust-design.md` sections 5, 6 and 9.

Repo-relative Kotlin paths are under `/home/blackbox/aniyomi-src`.

---

## 1. Why this needs its own page

The load pipeline decides what a user can run. Three rules are easy to get wrong when porting: the order of checks (an untrusted NSFW package is `Untrusted`, not `Error`), the all-or-nothing rule per package (one bad source class makes the package `Error`), and the trust key (it is per version and per signer, so every update needs a new trust decision unless the repo key matches).

---

## 2. Load pipeline as a decision table

Input: one package (`PackageManifest` plus file path). Output: `LoadResult`. The checks run top to bottom; the first failing check decides the result.

| Step | Check | Fails with | Kotlin reference |
|---|---|---|---|
| 1 | Label normalised (trim, strip `Tachiyomi: `) | never fails | `MangaExtensionLoader.kt:252-254` |
| 2 | `versionName` non-empty | `Error` (log `Missing versionName for extension …`) | `MangaExtensionLoader.kt:258-261` |
| 3 | `versionName` prefix parses to `major.minor` and is in `1.4..=1.5` | `Error` (log `Lib version is …`) | `MangaExtensionLoader.kt:264-271` |
| 4 | Signed (signer present and 32 bytes) | `Error` (log `Package … isn't signed`) | `MangaExtensionLoader.kt:273-277` |
| 5 | Trusted (repo fingerprint or per-version key) | `Untrusted(...)` | `MangaExtensionLoader.kt:277-288` |
| 6 | NSFW allowed or package not NSFW | `Error` (log `NSFW extension … not allowed`) | `MangaExtensionLoader.kt:290-294` |
| 7 | Every class or factory instantiates | `Error` (log `Extension load error …`) | `MangaExtensionLoader.kt:296-343` |
| 8 | Language derived | never fails | `MangaExtensionLoader.kt:346-353` |
| 9 | Success | — | `MangaExtensionLoader.kt:355-368` |

Ordering consequences (keep in tests, edge cases L10 to L13):
- An unsigned package is an error before trust is consulted, so it can never appear as `Untrusted`.
- An untrusted NSFW package is shown as untrusted, so the user can still decide to trust it; the NSFW gate applies after trust.

Kurobayomi version of step 3: the range is applied to the `abi` value that the manifest declares, and the version name must also match `^\d+\.\d+\.\d+$` at manifest validation. The Kotlin `toDouble()` quirk is not reproduced (see behaviour 3.8 step 4).

---

## 3. Selection between copies of one package

Aniyomi has two copies that can coexist: shared (system) and private (app data). Kurobayomi has one store, so selection is trivial. The rule that remains is the install-time rule.

| Situation | Kotlin result | Kurobayomi result |
|---|---|---|
| Only private copy | private | the file |
| Shared and private, shared version code higher or equal | shared | not applicable (dropped, T-18a-042) |
| Shared and private, private higher | private | not applicable |
| Install over installed, lower version code | refused (`Downgrade`) | refused (`Downgrade`) |
| Install over installed, equal version code | accepted | accepted |
| Install over installed, new signer does not contain old signer | refused (`SignerMismatch`) | refused (`SignerMismatch`) |

Store identity: one file per package name, `<pkg>.kuroext`, read-only. The file name is the lookup key, so a file whose manifest `pkg` differs from its name is rejected (edge case L2).

---

## 4. Signer and trust model

### 4.1 What the signer is
- Aniyomi: the APK certificate. The digest is the SHA-256 of the certificate bytes (`MangaExtensionLoader.kt:408-422`).
- Kurobayomi: an ed25519 key pair held by the repo owner. The digest is SHA-256 of the 32-byte **public key**. The package carries `signerPub`, written by the installer only after verification (design 9.4).

### 4.2 Trust decision (`TrustMangaExtension.isTrusted`)
```
trusted(pkg, versionCode, signer) =
     exists repo r: r.signing_key_fingerprint == hex(signer)
  OR "pkg:versionCode:hex(signer)" in app_state.trusted_extensions
```
Observed Aniyomi rule: the repo branch checks every digest of the package, the per-version branch checks only the last one. With one signer both branches check the same value.

### 4.3 Trust action (`TrustMangaExtension.trust`)
```
trusted_extensions := { k in trusted_extensions | not k.startsWith("pkg:") } ∪ { "pkg:versionCode:hex(signer)" }
```
Effect: at most one trusted version per package. Installing a newer version later needs a new trust unless the repo branch matches. This is why a user who trusted version 13 sees version 14 as untrusted (edge case T2).

### 4.4 Revoke
`revoke_all` clears only `trusted_extensions`. Repo-based trust is not affected, because it depends on the repo list and not on user decisions (edge case T4).

### 4.5 Prefix hazard
The filter uses `startsWith("pkg:")` with the colon. Without the colon, trusting `a.b` would also remove the entry of `a.b.c`. The Rust test in `p1-edge-cases-and-tests.md` 12.2 pins this (edge case T3).

### 4.6 Where the decision is used
- Load (step 5 above).
- Install result: an install from a repo whose key matches is trusted at once; from an unknown key the package is listed as untrusted and the user sees the dialog (ui-parity section 5).

---

## 5. Repo fingerprint lifecycle

| Event | Table state | Effect on trust |
|---|---|---|
| User adds repo; `repo.json` fetched | row with real fingerprint | repo branch trusts its packages |
| Repo migrated from Aniyomi preference | row with `NOFINGERPRINT-<n>` | no package matches until a refresh |
| Refresh returns the same fingerprint | row unchanged | unchanged |
| Refresh returns `NOFINGERPRINT` row's real fingerprint | row replaced | repo branch begins to trust its packages |
| Refresh returns a different fingerprint for a real row | ignored (no write) | unchanged; the old key stays |
| Two repos with the same fingerprint | UNIQUE violation on insert | second repo cannot be added (edge case X19) |

Design rule (behaviour 9.2): a fingerprint never silently moves to a new key for an existing repo. A key change needs a new repo entry, which the user adds deliberately.

---

## 6. Failure modes and their UI

| Failure | Result | What the user sees |
|---|---|---|
| Package has no signer (unsigned) | `Error` | Row not shown in installed; log entry only (Aniyomi shows nothing) |
| Package untrusted | `Untrusted` | Row under Installed with `UNTRUSTED` tag and Trust button |
| Source instantiation fails | `Error` | Row not shown; log entry |
| Lib version out of range | `Error` | Row not shown; log entry |
| Manifest invalid | `Error` | Row not shown; log entry |

Kurobayomi improvement (proposal, task T-18a-054): keep `Error` and `Untrusted` load results with their reason in the store, so the UI can show a "Problems" entry in the extension details. Aniyomi shows nothing; this is a deliberate addition for a single-user app. The display is area:17-source-app-layer's.

---

## 7. Concurrency

- Loading is per package, in parallel (design section 4). The per-package trust check reads the repo table through `spawn_blocking`.
- `trust` takes the per-package mutex before writing `trusted_extensions` and reloading (edge case R3). This prevents a reload racing a trust write.
- The store is only changed on the GTK thread, through `ExtEvent` (design section 5.3).

---

## 8. Test cases that pin this topic

- Load order (edge cases L10 to L13).
- Lib version boundaries (edge cases L5 to L9).
- Trust prefix (edge case T3), version replacement (T2), repo branch (T1), revoke (T4).
- Downgrade, equal version, signer mismatch (edge cases I1 to I3).
- Repo fingerprint lifecycle (edge cases X16 to X19).

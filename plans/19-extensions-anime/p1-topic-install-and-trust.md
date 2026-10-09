# Topic: install pipeline, package kinds, queue, and trust

Deep dive for area:19-extensions-anime. Covers the Aniyomi installer as observed (download, placement, dispatch, cancel, queue), the Linux install pipeline that replaces it, the package kinds shown in the store, and the trust rule. Sources are under `/home/blackbox/aniyomi-src`. Behaviour IDs refer to `p1-behavior.md`. Rust types are in `p1-rust-design.md` sections 4 and 6.

## 1. Pipeline overview

Aniyomi, one entry (behavior 8.2 to 8.5):

```
user taps Install
  -> DownloadManager.enqueue(apkUrl)                     [INST:93]
  -> poll every 1 s until SUCCESSFUL or FAILED           [INST:128-151]
  -> ACTION_DOWNLOAD_COMPLETE broadcast                  [INST:262-320]
  -> installApk(uri) dispatch by extension_installer     [INST:158-205]
       LEGACY        -> system installer Activity (dropped)
       PACKAGEINSTALLER / SHIZUKU -> foreground service queue (dropped)
       PRIVATE       -> copy to cache, installPrivateExtensionFile (kept as Linux placement)
  -> step Installed or Error
```

Kurobayomi, one entry (design section 6):

```
user clicks Install
  -> download (reqwest stream) to <pkg>-<versionCode>.kuroext.part   [download.rs]
  -> verify sha256, then ed25519, then store signer                   [verify.rs]
  -> place: downgrade check, signer continuity, fsync, rename, 0444   [place.rs]
  -> step Installed, manager registers the package from manifest      [manager.rs]
```

Steps in the UI: `Pending`, `Downloading`, `Installing`, `Installed`, `Error`, `Idle` (behavior 1.3). Kurobayomi emits `Installing` for the verify and place phase. Aniyomi's private path never emits it (behavior 8.10).

## 2. Downloads

| Aspect | Aniyomi | Kurobayomi |
|---|---|---|
| Transport | Android `DownloadManager` (OS service) | `reqwest` streaming GET |
| Location | `Environment.DIRECTORY_DOWNLOADS` under external files (INST:86-90) | `$XDG_CACHE_HOME/kurobayomi/ext-download/<pkg>-<versionCode>.kuroext.part` |
| Progress | none in the app; the OS shows a notification (`VISIBLE_NOTIFY_COMPLETED`, INST:91) | `Downloading` step and a byte counter for the page |
| Status polling | `delay(1.seconds)` (INST:147) | none. Progress is pushed |
| Parallelism | one DM request per install call; no cap | parallel, no cap for downloads; placement is serialised |
| Size limit | none | 64 MiB (Proposal) |
| Cancel | `downloadManager.remove(id)` (INST:212), which also deletes the file | cancel token; partial file deleted |
| Retry | none | none. The user presses Install again |

Downloads are not queued in Aniyomi. Every `downloadAndInstall` call starts a DM request immediately (INST:71-97). Only the install step is queued, and only for `PACKAGEINSTALLER` and `SHIZUKU`.

## 3. Verification

Order (Kurobayomi, design 4.2). Each step stops the pipeline on failure, and the partial file is deleted.

1. SHA-256 of the whole file equals `resources.kuroext.sha256` (64 lowercase hex). Failure: `DigestMismatch`.
2. `resources.kuroext.publicKey` decodes to 32 bytes. Failure: `BadKey`.
3. `hex(sha256(publicKey)) == store.signingKey`. Failure: `UntrustedSigner`. The signature is not checked, so a valid signature from another key is still refused.
4. `ed25519` verifies with `verify_strict` over the **raw 32-byte digest**, not over the file. Failure: `BadSignature`.
5. The manifest in the archive is validated (design 4.1). Failure: `Manifest`.
6. `manifest.pkg` equals the store `packageName`. Failure: `PkgMismatch`.

Why the digest is signed and not the file: the signature is stored in the index. A repo can re-sign without re-zipping (area:18-extensions-manga section 7.4).

Aniyomi has no equivalent. The APK signature is checked by Android at install time, and the signer is read from the package (LDR:409-423). On Linux, there is no installer that checks signatures, so the check is explicit.

## 4. Placement

Mirrors `installPrivateExtensionFile` (behavior 3). Kurobayomi adds the atomic write.

| Step | Aniyomi | Kurobayomi |
|---|---|---|
| Parse package | `getPackageArchiveInfo` (LDR:66-70) | `zip` open and manifest read |
| Current package | best of shared and private (LDR:71-74) | the installed file for the pkg, if any |
| Downgrade | refuse when `new < current` (LDR:77-82) | refuse when `new.version_code < current.version_code` |
| Reinstall | equal version allowed | equal version allowed |
| Signer | `containsAll(current signatures)` (LDR:90-93) | equal to current signer (single-signer model) |
| First install signer | not checked here (LDR:84-88 runs only when current exists) | checked by verification, always |
| Write | `target.delete()` then copy (LDR:100-102) | write `.part`, `fsync`, `rename` onto target |
| Mode | read-only (`copyAndSetReadOnlyTo`) | `0444` |
| Sidecar | none (signer read from APK) | `<pkg>.signer` file with the digest |
| Event | `notifyAdded` or `notifyReplaced` (LDR:103-107) | `Event::Installed` |

**Why signer continuity is an equality:** Kurobayomi has one signing key per package. Aniyomi's `containsAll` allows rotation, where the new APK is signed by both the old and the new key. The ed25519 model has no such multi-signature container, so rotation needs a new package name. This is a deliberate narrowing (recorded in behavior 3 and in T-19a-030).

## 5. Package kinds in the store (listing rules)

Every entry in a store is classified when the store is read (design 3.5, data 8.3). The classification controls what the user can do with it.

| Kind | Condition | Listed in | Install | Update | Notes |
|---|---|---|---|---|---|
| `Wasm` | `resources.kuroext` present and every field valid | Available (installable) and Installed | yes | yes, when `version_code` or `abi` is newer | The Kurobayomi entry |
| `Unsigned` | `kuroext` present, `ed25519` or `publicKey` missing or malformed | Available, badge `Unsigned package` | disabled, tooltip explains | never counted as an update | Area:18 calls this `Untrusted` (reconciled in T-19a-013) |
| `ApkOnly` | no `resources.kuroext`, or `kuroext` with a malformed `sha256` or `url` | Available, badge `Not runnable on Linux` | disabled, tooltip explains | never counted as an update | Aniyomi APK entries |

Rules:
- An `ApkOnly` entry never becomes `Installed`, so it can never be `Untrusted` or `Obsolete`.
- An entry with a `kuroext` block but no `ed25519` (or no `publicKey`) is `Unsigned`. Area:18 section 16.2 lists the same case as `Untrusted`. The anime Available model has no Untrusted flag, so `Unsigned` is used. T-19a-013 aligns the name with area:18-extensions-manga when that area freezes.
- Default view: `ApkOnly` rows are hidden, and the store row shows a count `N entries not runnable on Linux` (Proposal, preference `show_not_runnable` default false).

## 6. Queue and cancel

Aniyomi queue (behavior 9): used only for `PACKAGEINSTALLER` and `SHIZUKU`. FIFO, one active entry, `AtomicReference` for the active one.

Kurobayomi queue (design 6.2):
- One FIFO of entries. One install runs at a time (placement is serialised by a mutex). Downloads run in parallel.
- `cancel(pkg)` fires the entry's token.
  - Queued entry: removed and set to `Idle`.
  - Downloading entry: request aborted, partial file deleted, `Idle`.
  - Verifying or placing entry: the placement is not interrupted once `rename` has started. Verification can be interrupted at a chunk boundary. The result is `Installed` if placement finished, else `Idle`.
- Shutdown: queued entries become `Error` (same as Aniyomi, behavior 9). The active entry is cancelled and becomes `Idle` (Aniyomi leaves it, a defect).

The cancel defect that this fixes (behavior 8.6): cancel during download leaves the UI row in `Downloading` forever, because no `Idle` is emitted and the merged flow never completes. Test TST-19-075.

## 7. Uninstall

| Case | Aniyomi | Kurobayomi |
|---|---|---|
| System-installed package | `ACTION_UNINSTALL_PACKAGE` (INST:221-226) | not applicable |
| Private package | delete file, `notifyRemoved` (INST:227-230) | delete `<pkg>.kuroext` and `<pkg>.signer`, emit `Removed` |
| Unknown package | same as private (file absent) | delete is `Ok` when the file is absent |

After removal, the manager removes the pkg from installed and untrusted maps (behavior 5.11). Kurobayomi also clears the pkg's step entry.

Uninstall of a package whose store still lists it: the package is listed as Available again on the next refresh (no change to the store).

## 8. Trust

Two rules, either one is enough (behavior 6.2, design 4.3):

1. **Repo rule.** The package's signer digest equals the `signingKey` of a stored store. Trusts every version of every package that store signs.
2. **Per-version rule.** The entry `"<pkg>:<versionCode>:<digest>"` is in `trusted_extensions`. Trusts one version.

Storage: `trusted_extensions` (app state, `__APP_STATE_trusted_extensions` in Aniyomi, data section 4). Kurobayomi stores it in the app-state file (area:02-preferences-system). The format is unchanged. Entries from Aniyomi backups do not match (their digest is an APK certificate hash), so they are harmless.

Trust prompt: shown for `Untrusted` rows (behavior 6.4). `Trust` writes the per-version entry and reloads the package. `Uninstall` deletes the file.

Revoke: `revokeAll` deletes the per-version set and keeps repo trust. The settings action also revokes manga trust and shows `Requires app restart to take effect` (behavior 6.3).

## 9. Failure matrix

| Failure | Step | Result | Message (Kurobayomi) |
|---|---|---|---|
| HTTP non-2xx | download | `Error` | `Failed to install <name>` |
| Network drop | download | `Error`, partial deleted | same |
| Body too large | download | `Error`, partial deleted | `Extension package is too large` |
| Digest mismatch | verify | `Error` | `Extension package is corrupt` |
| Bad key or signature | verify | `Error` | `Extension signature is invalid` |
| Signer not the store's | verify | `Error` | `Extension is not signed by this store` |
| Manifest invalid | place | `Error` | `Extension manifest is invalid` |
| Downgrade | place | `Error` | `Installed extension version is higher. Downgrading is not allowed.` (log text) |
| Signer changed | place | `Error` | `Installed extension signature is not matched.` (log text) |
| Disk full | place | `Error`, `.part` deleted | `Not enough disk space` |
| Cancelled | any | `Idle` | none |

The user-visible messages above are new strings (not in Aniyomi). They go in the same i18n file as the rest of the app (area:02-preferences-system keys). The log texts are verbatim from Aniyomi (data section 11).

## 10. Test anchors

TST-19-033, 075 to 091, 092 to 099 (see `p1-edge-cases-and-tests.md`).

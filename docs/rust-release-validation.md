# Bundled Rust release validation

Validated on Windows x64 on 2026-10-16 using the complete `artifacts/runtime-storage-v1` runtime.

- Installer: `artifacts/installer/SupabaseLocal.Setup.exe`
- Size: 720,240,560 bytes
- SHA256: `ce433ded93f8771ff22e07ec7e2ea3ee5cca34ae2619f8ba9d15f29cb7820c64`
- Payload: 112,980 files plus the checksum manifest; approximately 1.75 GiB unpacked.
- Evidence: `work/rust-release-validation-20261016-v2/release-test-result.json` and adjacent logs.

## Passed checks

- Six Rust unit tests, including bundle corruption rejection, unsafe Windows paths, archive traversal, corrupt-file rollback, repeat installation, tamper rejection, project data preservation, NTFS junctions and hard links.
- Actual release executable SHA256 sidecar, bundled extraction, repeat verification, modified-file rejection, successful verification after restoring the file, and preservation of an existing data marker.
- Two full manager cycles using files extracted by the release executable: PostgreSQL, postgres-meta, Studio, Auth, PostgREST and Storage.
- Real Studio SQL creation/query and database persistence, Auth signup/login, gateway REST access and missing-key rejection, and stable project secrets across restart.
- Storage upload/download, copy/move, byte ranges, metadata, public/signed URLs, list/delete, RLS and restart persistence.
- Bundled desktop launcher startup, all six services ready, and graceful shutdown with no remaining owned listeners.
- Packaged GPUI native-window construction, explicit hidden-window frame rendering, clean process exit, and verification output.

The tests used isolated data under `work`. They did not register, replace or uninstall the current user's existing installation. Interactive button clicks and the real current-user uninstall flow were not exercised in this release run.

## Build fixes found during validation

The runtime archive is appended after linking with a length and SHA256 footer; passing hundreds of megabytes through Rust `include_bytes!` caused excessive compiler memory use. Fresh extraction now hashes bytes as they are written into the private staging directory. Repeat verification checks parent directories once per pass while continuing to check every file for unexpected reparse points and content changes. Setup explicitly quits after its last window closes and uses Windows window options that show the normal installer without taking focus.

## Reproduce

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-installer.ps1 -Runtime artifacts/runtime-storage-v1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-rust-release.ps1
```

Build output, isolated databases, generated secrets and detailed evidence stay in ignored directories. The executable is not committed to Git.

## Branding follow-up (2026-10-16)

The current installer restores the original project's PNG logo, Segoe UI header, "Supabase Local" title, "Unofficial native Windows port" subtitle, and unchanged six-frame Windows icon. The logo is decoded before rendering so it is present in the initial frame. All six native executable icon frames were compared byte for byte with the original ICO, and the packaged GPUI window render check passed.

- Branding build size: 720,333,744 bytes.
- Branding build SHA256: `4202a6e32945115bfb01c37151e940ab3341429411e53d3168b42e763792f002`.
- Runtime payload hash remains the previously tested version `a1fc3aa17625cfa3`; services were not rebuilt for this branding change.
- UI evidence: `work/rust-branding-packaged-ui.json` and its adjacent error log.

## Windows integration fix (2026-10-16)

The GUI Install button exposed a failure that extraction-only validation had missed: Rust kept the temporary PowerShell script open with a delete-on-close handle, preventing Windows PowerShell from reading it. The regression was reproduced with the exact file-in-use error. The helper now closes the handle before launching PowerShell, retains the temporary path until completion, captures stderr, and reports the failing prerequisite or registration stage.

All nine Rust tests pass, including temporary script execution, arguments with spaces, underlying error propagation and actual COM shortcut/registry creation in an isolated scope. The packaged executable's `--install` mode calls the same installation function as the GUI button and successfully installed into the real current-user scope. Both Start menu shortcuts and Installed apps metadata were verified, all 1,234 pre-existing project file hashes were unchanged, and the newly installed launcher started all six services using isolated test data and stopped every owned listener gracefully.

- Current installer size: 720,350,640 bytes.
- Current SHA256: `69bebb99ceae6e501d99cec04fc85fc11a08bcfe3271dd6f8ca8888de0868a5c`.
- Evidence: `work/rust-install-fix-validation-20261016/installation-result.json` and adjacent logs.
- This follow-up performed real current-user registration; the earlier isolated release run did not.

## Live installation log (2026-10-16)

The setup window now retains timestamped progress in an automatically scrolling log panel. It reports prerequisites, archive checks, file counts, runtime links, Windows registration, completion and detailed errors. All nine Rust tests and the packaged native-window render check pass.

The earlier installer was open during packaging, so this build is available at `artifacts/installer/with-logs/SupabaseLocal.Setup.exe`. Close the earlier setup window and use this executable for the live log.

- Log build size: 720,360,368 bytes.
- SHA256: `e526726dc705115ed8a6b108ccc6f5a8fea769d3515ab96dd509bf0fbc59d7bd`.
- UI evidence: `work/rust-install-log-ui.json`.

## Rust desktop launcher (2026-10-16)

The bundled desktop launcher now uses Rust / GPUI Kit, with the original logo, a service dashboard, activity log and native notification-area icon. Window and tray Restart/Exit actions share confirmation dialogs; closing the window also requires exit confirmation. The ASP.NET manager and separate C# uninstaller remain bundled.

- Installer: `artifacts/installer/rust-desktop/SupabaseLocal.Setup.exe`, 729,993,235 bytes.
- SHA256: `dbb183217a25fea8c79c9e9e783611452fc9e4c85c0e5912c348296889451d13`.
- All eleven Rust tests pass, including confirmation cancellation and duplicate-action gating.
- The complete packaged release passed extraction, repeat verification, tamper rejection, data preservation and two SQL/Auth/REST/Storage integration cycles.
- The extracted Rust launcher started all six services and shut down gracefully. A separate real restart test used distinct manager process IDs, preserved secrets and all four stored project files, and left no service listeners running.
- Native UI verification rendered the packaged dashboard, tray icon and both confirmation dialogs. Interactive mouse clicks were not exercised; confirmation dispatch is covered by unit tests and lifecycle behavior by the real process tests.
- Evidence: `work/rust-desktop-release-validation-20261016` and `work/rust-desktop-packaged-ui.json`.

Tests used isolated data. This build has not replaced the current user's installed launcher; install the new bundle after closing the older launcher.

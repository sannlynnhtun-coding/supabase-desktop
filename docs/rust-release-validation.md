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

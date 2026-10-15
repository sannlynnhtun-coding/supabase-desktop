# Rust installer

The setup executable is implemented in `src/SupabaseLocal.Setup.Rust` with GPUI Kit 0.7.3. The native service manager, installed desktop launcher, uninstaller, archive build tool, and upstream service builds retain their existing implementations. The original C# setup remains as a migration reference and for its existing packaging checks; the production packaging script builds Rust setup.

## Build

Install an MSVC Rust toolchain, Visual Studio C++ Build Tools with Windows SDK, and CMake. The runtime build also needs .NET 10 and Node 24.

```powershell
cargo check --locked --manifest-path src/SupabaseLocal.Setup.Rust/Cargo.toml
cargo test --locked --manifest-path src/SupabaseLocal.Setup.Rust/Cargo.toml
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-installer.ps1 -Runtime artifacts/runtime
```

The packaging script assembles the existing service payload, sets `SUPABASE_PAYLOAD`, and compiles it into the Rust executable. A development build without that variable opens the UI but reports the missing payload if installation is requested. Never distribute that development build as an installer.

Setup runs extraction and Windows integration on a worker thread and updates its UI through a channel. It verifies each manifest file, atomically moves a staging directory into the version directory, restores NTFS junctions and hard links, and registers current-user shortcuts and uninstall metadata. It preserves the existing installation ownership identifier so the bundled uninstaller can remove the installation. PowerShell handles Windows prerequisite and COM shortcut integration; the setup UI and payload installer are Rust.

Use `SupabaseLocal.Setup.exe --verify-install <empty-test-directory>` to exercise an embedded payload without registering an installation or starting services. Verify repeat installation and tamper rejection before release. This command writes `verification-result.json` on success. Close the setup window only after installation completes; interruption can leave a version that will be verified on the next attempt.

The installed C# desktop launcher owns service startup, shutdown and data-preserving uninstallation. Converting that launcher or the ASP.NET manager is a separate migration.

Framework documentation: https://gpui-kit.com/docs/getting-started/

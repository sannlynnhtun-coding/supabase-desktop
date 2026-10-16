# Rust installer

The setup executable is implemented in `src/SupabaseLocal.Setup.Rust` with GPUI Kit 0.7.3. The native service manager, installed desktop launcher, uninstaller, archive build tool, and upstream service builds retain their existing implementations. The original C# setup remains as a migration reference and for its existing packaging checks; the production packaging script builds Rust setup.

## Build

Install an MSVC Rust toolchain, Visual Studio C++ Build Tools with Windows SDK, and CMake. The runtime build also needs .NET 10 and Node 24.

```powershell
cargo check --locked --manifest-path src/SupabaseLocal.Setup.Rust/Cargo.toml
cargo test --locked --manifest-path src/SupabaseLocal.Setup.Rust/Cargo.toml
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-installer.ps1 -Runtime artifacts/runtime
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-rust-release.ps1
```

The packaging script assembles the existing service payload, compiles the optimized Rust executable, and appends the ZIP archive with a length and SHA256 footer. This keeps the service archive out of Rust optimization while producing one self-contained installer. The archive checksum and every manifest file are verified before use. An unpackaged Cargo build opens the UI but reports the missing payload if installation is requested. Never distribute that development build as an installer.

Setup runs extraction and Windows integration on a worker thread and updates its UI through a channel. It hashes each file while extracting into its private staging directory, checks each existing file on repeat installations, and atomically moves the staging directory into the version directory, restores NTFS junctions and hard links, and registers current-user shortcuts and uninstall metadata. It preserves the existing installation ownership identifier so the bundled uninstaller can remove the installation. PowerShell handles Windows prerequisite and COM shortcut integration; the setup UI and payload installer are Rust.

Use `SupabaseLocal.Setup.exe --verify-install <empty-test-directory>` to exercise an embedded payload without registering an installation or starting services. Verify repeat installation and tamper rejection before release. This command writes `verification-result.json` on success. Close the setup window only after installation completes; interruption can leave a version that will be verified on the next attempt.

The installed C# desktop launcher owns service startup, shutdown and data-preserving uninstallation. Converting that launcher or the ASP.NET manager is a separate migration.

`scripts/test-rust-release.ps1` tests the actual packaged executable in a fresh directory under `work`. It checks the release SHA256, embedded extraction, repeat verification, tamper rejection and data preservation; runs two manager integration cycles including real Studio SQL, Auth, REST and Storage; verifies the bundled desktop launcher's six-service startup and graceful shutdown; and checks native GPUI window creation and closing. The test requires the local service ports to be free. It retains its isolated test data and writes `release-test-result.json` alongside its logs. It does not register or uninstall the current user's existing installation.

`--verify-ui <result.json>` mounts the production setup view in a hidden native GPUI window, draws a frame, records success and exits. It neither installs nor registers anything. Hidden windows do not receive display-driven frames, so this mode explicitly draws the mounted view.

To repackage an already verified payload after a Rust-only change, run `scripts/package-rust-installer.ps1 -Payload <payload.zip>`. This produces the final EXE and SHA256 sidecar without copying or rebuilding upstream services. The final bundled EXE, rather than Cargo's unpackaged executable, is the release artifact.

Framework documentation: https://gpui-kit.com/docs/getting-started/

`SupabaseLocal.Setup.exe --install` performs the real current-user installation using the same function as the GUI Install button, including prerequisite checks and Windows registration. Unlike `--verify-install`, it creates Start menu and Installed apps entries. `scripts/test-rust-current-user-install.ps1` exercises this path, verifies the registered shortcuts and uninstall command, checks existing project file hashes, and tests the installed launcher with isolated service data. Run it only after closing the local stack.

The PowerShell helper closes its temporary script file before launching PowerShell while retaining the temporary path until the process exits. PowerShell errors are captured and displayed with the failing stage. Regression tests exercise locked-script prevention, arguments containing spaces, actionable error details, and real shortcut/registry registration in an isolated scope.

During installation the window shows a live, timestamped installation log. It retains prerequisite checks, archive verification, extraction or repeat-verification counts, runtime-link restoration, Windows registration, completion and error messages. New entries scroll into view automatically; earlier entries remain available by scrolling. A retry starts a new log. Detailed errors stay in the log below the short status message.

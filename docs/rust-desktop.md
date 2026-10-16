# Rust desktop launcher

The installed `SupabaseLocal.App.exe` is a Rust / GPUI Kit application. Its dashboard shows the six supported native services, loopback ports, readiness, a timestamped activity log, and the project data location. The original Supabase logo and Windows icon are reused.

## Window and notification area

- **Open Studio** opens the local Studio once ready.
- **Hide to tray** hides the window while services keep running. Double-click the notification icon or choose **Show Supabase Desktop** to restore it.
- **Restart services** opens **Confirm restart**, with **Restart services** and **Cancel** buttons. Confirmation gracefully stops the owned manager and starts a new one using the same data directory.
- **Exit**, including the window close button, opens **Confirm exit**, with **Exit Supabase Desktop** and **Cancel** buttons. The app quits only after its owned manager has stopped.
- The notification-icon menu offers Show, Open Studio, Restart and Exit. Its Restart/Exit entries use the same confirmation flow as the dashboard. Cancel never sends a service-control command.

Confirmed actions are gated while in progress, preventing duplicate restarts or overlapping shutdowns. A shutdown failure keeps the launcher available and reports the error instead of forcibly killing services. A second launcher cannot acquire the installation lock or take over another manager.

## Implementation and packaging

`src/SupabaseLocal.Setup.Rust/src/app.rs` owns the GPUI window, tray menu and confirmation dialogs. `app_runtime.rs` owns the manager process and authenticated loopback shutdown requests. Blocking startup/status/shutdown work runs on a background thread. Restart keeps the project database, files and generated secrets.

The ASP.NET Core gateway/service manager remains unchanged. The original C# uninstaller is bundled separately as `SupabaseLocal.Uninstall.exe`; the Rust launcher forwards the existing uninstall entry to it. The original C# launcher source remains as a reference, but packaging installs the Rust launcher.

The normal `scripts/build-installer.ps1` builds and packages the Rust app. To reuse an already verified service archive without copying 112,980 runtime files again:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/refresh-rust-desktop.ps1 -Payload <existing-payload.zip>
```

The refresh preserves the prior C# desktop binary as the uninstaller, replaces the launcher, recalculates their manifest hashes, and bundles the updated ZIP. It does not change the upstream service files.

## Verification

```powershell
cargo test --locked --manifest-path src/SupabaseLocal.Setup.Rust/Cargo.toml
SupabaseLocal.App.exe --verify-ui <result.json>
SupabaseLocal.App.exe --verify-restart <isolated-data-directory>
```

UI verification constructs the native tray icon, renders the dashboard and both confirmation dialogs, and exits without starting services. Unit tests cover cancellation, missing confirmation, duplicate confirmation and distinct restart/exit prompts. Lifecycle verification requires all local stack ports to be free and no other launcher running; it starts six services, gracefully stops them, starts a new manager using the same isolated data, and stops again. These command-line verification modes are test harnesses; ordinary window and tray actions always require confirmation.

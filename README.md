# Supabase Desktop

<img src="src/Shared/Branding/supabase-logo.svg" alt="Supabase logo" width="48" height="50">

An experimental native Windows port of upstream Supabase services, with a Rust / GPUI Kit installer and ASP.NET Core manager. No Docker or WSL2.

The Rust setup lives in `src/SupabaseLocal.Setup.Rust`. The installer, desktop launcher and uninstaller use [GPUI Kit](https://gpui-kit.com/). The launcher provides service status, an activity log, a notification-area icon and confirmed Restart/Exit actions. The ASP.NET Core service manager remains bundled. The uninstaller is built in Rust and preserves project data by default. See [Rust installer development](docs/rust-installer.md) for build and verification steps. The original C# setup is retained as a migration reference.

## Runtime decision

The official Supabase local stack cannot currently run natively on Windows without a container engine. Its experimental native runtime supports Linux amd64/arm64 (Ubuntu 22.04 or later / glibc 2.35+) and Apple-silicon macOS.

The current milestone runs the real Supabase Studio, upstream postgres-meta, Supabase Auth, PostgREST and Supabase Storage against private Windows PostgreSQL. These are upstream services, not C# replacements. This is an unofficial port; it does not make `supabase start` support a native Windows runtime.

The app is named **Supabase Desktop**. Internal project names, the `%LOCALAPPDATA%/SupabaseLocal` data directory and the uninstall registration ID are retained for compatibility with existing installations.

## Use the installer

Open `SupabaseDesktop.Setup.exe` and select **Install Supabase Desktop**. The Rust installer checks Windows prerequisites and installs a verified bundled runtime under `%LOCALAPPDATA%/SupabaseLocal/versions`. Select **Open Supabase Desktop** after installation; the desktop launcher initializes the database, starts the native processes and shows their status. Select **Open Studio** to open real Supabase Studio at `http://127.0.0.1:54323`.

The desktop launcher stays available in the Windows notification area. **Hide to tray** keeps services running; double-click the icon or choose **Show Supabase Desktop** to restore the window. **Restart services** asks **Confirm restart** before interrupting connections. **Exit**, including closing the window, asks **Confirm exit** and then stops services gracefully. Cancel leaves services running. Open **Supabase Desktop** from Start to restart the same project. The database, generated secrets, SQL snippets and logs are retained under `%LOCALAPPDATA%/SupabaseLocal/data`. No global PATH modification is needed. The .NET and Node runtimes are bundled; users do not need SDKs, Git or npm.

Target: Windows 10/11 x64 and NTFS. If the required Microsoft Visual C++ runtime is absent, the launcher downloads a checksum-pinned package from Microsoft and requests the normal Windows administrator prompt. That first prerequisite installation needs internet access and can require a restart. Redistribution of that Microsoft package is not included in this repository or payload.

Setup creates **Supabase Desktop** and **Uninstall Supabase Desktop** entries in your Windows Start menu. After installation, search Start for **Supabase Desktop** to open the installed desktop app; you do not need to keep the downloaded setup EXE. Installation is for the current Windows user.

The bundled runtime is large, so allow time for the first installation and integrity checks. Later starts from the Start menu reuse the installed files.

To uninstall, close Supabase Desktop and use **Settings → Apps → Installed apps → Supabase Desktop → Uninstall**, or its Start menu uninstall entry. Uninstall removes application versions and shortcuts while keeping project data by default. Selecting **Also permanently delete my local project data** requires an additional confirmation. Reinstalling can reuse retained data.

The installer executable is a generated artifact under `artifacts/installer`, not committed into Git. See [Rust desktop launcher](docs/rust-desktop.md) for dashboard, notification-area controls and verification details.

## Verified scope

| Component | Status |
| --- | --- |
| Real Supabase Studio | Native production build; SQL creation/query and Table Editor row edit verified in the browser |
| PostgreSQL 17.6 | Native private cluster; authenticated loopback connections, persistent data, startup/shutdown verified |
| postgres-meta | Windows asset-build fix; health, SQL and table introspection verified |
| Auth v2.197.0 | Windows listener port; upstream migrations, email/password signup and login verified |
| PostgREST 16.4 | Windows executable with private libpq; JWT and anonymous/cross-user RLS isolation verified |
| C# gateway | `/auth/v1`, `/rest/v1` and `/storage/v1`; local CORS; Auth/REST API-key enforcement; Storage JWT/public/signed routes |
| Storage v1.60.4 | Upstream service with NTFS metadata adapter; upload/download, copy/move, list/delete, range requests, signed/public URLs and RLS verified across restarts |
| Realtime, Edge Functions, analytics, mail catcher, Supabase-specific extensions | Not ported or included |

Email confirmation is automatic for local Auth testing. SMTP and OAuth providers are not configured. Storage uses NTFS files under the private data directory and a 50 MiB upload limit. Its image transformation, S3 protocol, vector and analytics features are disabled; resumable uploads are not validated. Realtime subscriptions, Edge Functions and full Supabase Postgres extension parity remain unavailable. Studio can show upstream navigation for unavailable features; the manager reports the supported subset at `http://127.0.0.1:54320`.

References:

- https://supabase.com/docs/guides/local-development
- https://supabase.com/docs/guides/local-development/docker-and-native-runtimes

## Development

For development, install the Rust MSVC toolchain, Visual Studio C++ Build Tools, CMake, the .NET 10 SDK, Git and Node 24/npm. To build only the manager:

```powershell
dotnet build
dotnet run --project src/SupabaseLocal
```

To build the bundled installer from pinned sources:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-native.ps1
```

Build inputs and downloads stay in ignored `work/`; service source revisions and archive hashes are in `scripts/sources.lock.json`. PostgreSQL, Node, Go and PostgREST downloads are verified before extraction. Studio uses its pinned lockfile and a direct Next build to avoid POSIX wrappers. Build scripts preserve upstream licenses and relocate Windows dependency links to per-install NTFS junctions.

For an existing completed Studio build, `-SkipStudioBuild` reuses it. To package an already assembled runtime, use `scripts/build-installer.ps1 -Runtime <runtime-directory>`.

## Checks

```powershell
dotnet run --project tests/Installer.Checks
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-pgmeta.ps1 -WithApiServices -TestDirectory work/fresh-api-test
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-manager.ps1 -Manager <published-manager.exe> -Runtime <runtime-directory> -Data work/fresh-manager-test -WithStorage
```

Installer checks cover integrity, traversal rejection, repeat install, data preservation and NTFS links. API checks use a disposable private test cluster. The manager check runs two complete cycles, verifies real Studio SQL and gateway signup/login, checks secret/data persistence and confirms all owned listeners stop. `--verify-install <directory>` on a generated setup executable validates its actual embedded payload without opening UI or starting services.

See `docs/native-windows.md` for remaining service-port work.

## Commits

Use Conventional Commits for each completed task, such as `chore: scaffold ASP.NET Core 10 application` and `feat: add local stack management`.

Git remote: https://github.com/sannlynnhtun-coding/supabase-desktop

## MIT License

This project is licensed under the [MIT License](LICENSE).

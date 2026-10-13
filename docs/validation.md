# Native Windows milestone validation

Validated on the development Windows desktop on 2026-10-12, with desktop installation checks added on 2026-10-13. No Docker or WSL commands were used to run the stack.

## Results

- `dotnet build`: all four solution projects build with zero warnings and errors. The C# archive tool also compiles and verifies every file while packaging.
- `tests/Installer.Checks`: extraction, repeat install, installed-file tampering, archive traversal, corrupt-payload rollback, user-data preservation, real NTFS junction creation/removal and escaping-link rejection pass.
- `scripts/test-pgmeta.ps1 -WithApiServices`: native PostgreSQL initialization, SQL create/insert/select, table discovery, upstream Auth migrations/signup/password login, PostgREST JWT validation, service-role insertion and anonymous/cross-user RLS isolation pass.
- Upstream Studio production build succeeds. In the browser, Studio created a table through SQL Editor, returned its row and saved a change through Table Editor. A screenshot is provided with the installer deliverable.
- Manager lifecycle checks pass: persistent database contents and Auth users, stable generated secrets, standard gateway paths, rejection of missing API keys, and shutdown of all owned listeners.
- A transient PostgREST exit code 3 during rapid startup was reproduced. Bounded startup retry recovered it. Four consecutive cycles then passed, including a retry.
- The earlier core setup executable extracted and verified all 34,563 files and restored its package junctions successfully.
- The earlier installed self-contained manager passed two further cycles with a data path containing spaces. PATH contained only Windows system directories, excluding global Node, Git and .NET tools. The installed manager DLL hash matched the tested self-contained build.
- The Storage NTFS metadata adapter passes write/read, absent metadata, unchanged object content, copy retention and invalid attribute-name checks.
- Storage v1.60.4 compiles natively with Node 24.15.0. A clean dependency installation with bounded download concurrency succeeds. The production dependency tree contains the upstream Knex patch and is assembled separately from development dependencies.
- Storage passes two cycles against its development build and two against its assembled production runtime, including a data path with spaces: bucket creation, upload, copy/move, authenticated download, range requests, persisted content type/ETag/cache headers, list/delete, public and signed URLs, anonymous rejection, owner access and other-user rejection. The tests also check that migrations do not grant the PostgreSQL superuser role to the REST authenticator.
- All 32 relevant upstream file-backend and secure-path tests pass after applying the Windows patch. The filename guard preserves the upstream traversal error contract.
- The 0.2.0 desktop installer extracted and verified all 112,974 files from its actual embedded payload into the normal per-user installation directory. Its separate self-contained desktop EXE started PostgreSQL, postgres-meta, Auth, PostgREST, Storage and real Studio with only Windows system paths, then stopped its services.
- That installed runtime passes two further manager cycles with Studio SQL, Auth/REST and Storage checks. Storage explicitly rejects a modified JWT signature and a second authenticated user's request for another user's private file. All owned listeners stop after both cycles.
- Actual Start menu shortcuts target the installed desktop EXE, and the current-user Installed apps entry contains its quoted uninstall command. Isolated installer checks verify repeat registration, rejection while the desktop app is running, deletion of internal/external junctions without following targets, default data preservation, explicit project-data removal and removal of shortcuts/registry entries. The real deferred-cleanup PowerShell script also passes a private temporary-folder check.
- Realtime v2.102.3 builds into a native Windows release with checksum-pinned Erlang/Elixir/Rebar and an unchanged upstream dependency lock. It is excluded from the installer pending runtime and replication validation.

- Official Supabase logos now identify the Windows setup, desktop launcher, uninstaller, manager page, favicons and README. Six unchanged upstream PNG sizes are packaged into the Windows EXE icon. Launcher and uninstall previews were checked at the current Windows display scale; installer regression checks pass. The branded archive passes SHA-256 verification for all 112,980 packaged files. The prior installed version was absent, so this branding update supplies a rebuilt installer without reinstalling the app. Cleanup of the incomplete branding staging directory in the user installation versions folder was blocked by automatic approval review.

## Artifact

`SupabaseLocal.Setup.exe` SHA-256:

```text
66cdba1092bd1c1eadd435a42705bea04f044d5efd598893524a0c8f3fd0e265
```

The generated executable is not stored in Git. Build it with `scripts/build-native.ps1`, or use the local installer deliverable supplied with this milestone.

## Limits

This is a partial Windows Supabase distribution. Realtime, Edge Functions, analytics/mail services and Supabase-specific extension parity are not validated or included. Storage image transformation, S3 protocol, vector and analytics features are disabled; resumable uploads are not validated. A clean Windows VM has not been tested. The desktop already had a supported Microsoft C++ runtime, so the launcher's missing-runtime download/UAC path has not been executed; its pinned vendor package was downloaded separately and its Microsoft Authenticode signature verified.

Temporary build inputs remain under ignored `work/` and `artifacts/`. Optional cleanup was blocked by automatic approval review (`blocked by policy`).

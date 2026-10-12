# Native Windows milestone validation

Validated on the development Windows desktop on 2026-10-12. No Docker or WSL commands were used to run the stack.

## Results

- `dotnet build`: all three projects build with zero warnings and errors.
- `tests/Installer.Checks`: extraction, repeat install, installed-file tampering, archive traversal, corrupt-payload rollback, user-data preservation, real NTFS junction creation/removal and escaping-link rejection pass.
- `scripts/test-pgmeta.ps1 -WithApiServices`: native PostgreSQL initialization, SQL create/insert/select, table discovery, upstream Auth migrations/signup/password login, PostgREST JWT validation, service-role insertion and anonymous/cross-user RLS isolation pass.
- Upstream Studio production build succeeds. In the browser, Studio created a table through SQL Editor, returned its row and saved a change through Table Editor. A screenshot is provided with the installer deliverable.
- Manager lifecycle checks pass: persistent database contents and Auth users, stable generated secrets, standard gateway paths, rejection of missing API keys, and shutdown of all owned listeners.
- A transient PostgREST exit code 3 during rapid startup was reproduced. Bounded startup retry recovered it. Four consecutive cycles then passed, including a retry.
- The final setup executable's actual embedded payload extracted and verified all 34,563 files and restored its package junctions successfully.
- The final installed self-contained manager passed two further cycles with a data path containing spaces. PATH contained only Windows system directories, excluding global Node, Git and .NET tools. The installed manager DLL hash matched the tested self-contained build.
- The Storage NTFS metadata adapter passes write/read, absent metadata, unchanged object content, copy retention and invalid attribute-name checks. The full Storage service remains excluded because dependency downloads failed.

## Artifact

`SupabaseLocal.Setup.exe` SHA-256:

```text
dd7a8328d7c0b90dd75b24c34f19448f34326d8e06808296b39785a6a4f7e18a
```

The generated executable is not stored in Git. Build it with `scripts/build-native.ps1`, or use the local installer deliverable supplied with this milestone.

## Limits

This is a core-services prototype, not a complete Windows Supabase distribution. Realtime, Edge Functions, Storage APIs, analytics/mail services and Supabase-specific extension parity are not validated or included. A clean Windows VM has not been tested. The desktop already had a supported Microsoft C++ runtime, so the launcher's missing-runtime download/UAC path has not been executed; its pinned vendor package was downloaded separately and its Microsoft Authenticode signature verified.

Temporary build inputs remain under ignored `work/` and `artifacts/`. Optional cleanup was blocked by automatic approval review (`blocked by policy`).

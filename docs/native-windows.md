# Native Windows port

## Contract

Run upstream Supabase services as Windows processes, without Docker, WSL, or a substitute Studio. C#/.NET 10 owns installation, configuration, lifecycle and browser launch. The services retain their upstream languages and licenses.

This is an unofficial experimental port, not an upstream-supported Supabase CLI runtime.

## Service plan

| Service | Windows strategy | Initial status |
| --- | --- | --- |
| PostgreSQL | EDB Windows binary distribution, private data directory, loopback only | Integration target |
| Studio | Build actual upstream Studio using Windows Node; bypass POSIX npm wrappers | Integration target |
| postgres-meta | Build upstream TypeScript and run on Windows Node | Integration target |
| PostgREST | Upstream Windows release executable | Integration target |
| Auth | Compile upstream Go source for Windows; execute database migrations | Build investigation |
| Storage | Build upstream Node service; native filesystem adapter | Build investigation |
| Realtime | Windows Erlang/Elixir build, migration and replication tests | Not verified |
| Edge Runtime | Investigate upstream Rust/Deno Windows build | Not verified |
| Gateway | ASP.NET Core reverse proxy with equivalent routes and API key handling | Not implemented |
| Supabase Postgres extensions | Build required extensions individually against Windows PostgreSQL | Not verified |

Plain PostgreSQL is not equivalent to the Supabase Postgres distribution. Features depending on unavailable extensions must remain explicitly unavailable.

## Acceptance

An installer must not report full Supabase readiness based only on Studio's HTTP response. Verify SQL execution through postgres-meta, table editing through Studio, Auth signup/token flow, REST permissions, file upload/download, Realtime subscriptions and Edge execution separately. Preserve user data on upgrade and stop. Verify downloaded payload SHA-256 hashes. Pin source revisions and preserve licenses. Install without global PATH modifications.

## Upstream references

- https://supabase.com/docs/guides/local-development/docker-and-native-runtimes
- https://github.com/supabase/supabase/tree/master/apps/studio
- https://github.com/supabase/postgres-meta
- https://github.com/PostgREST/postgrest/releases
- https://github.com/supabase/auth

The official native runtime currently excludes Windows. This project investigates direct Windows builds rather than invoking that runtime.

# Supabase Local for Windows

ASP.NET Core 10 application foundation for a Windows installer and local Supabase manager.

## Runtime decision

The official Supabase local stack cannot currently run natively on Windows without a container engine. Its experimental native runtime supports Linux amd64/arm64 (Ubuntu 22.04 or later / glibc 2.35+) and Apple-silicon macOS.

The project requires native Windows execution: Docker and WSL2 are excluded. The implementation scope is pending: either build a Supabase-inspired ASP.NET Core backend with explicitly defined compatibility, or investigate porting the upstream services to Windows. This repository does not yet contain a working Supabase installer.

References:

- https://supabase.com/docs/guides/local-development
- https://supabase.com/docs/guides/local-development/docker-and-native-runtimes

## Development

Install the .NET 10 SDK, then run:

```powershell
dotnet build
dotnet run --project src/SupabaseLocal
```

The application currently contains the standard ASP.NET Core empty-template endpoint.

## Commits

Use Conventional Commits for each completed task, such as `chore: scaffold ASP.NET Core 10 application` and `feat: add local stack management`.

Git remote: https://github.com/sannlynnhtun-coding/supabase-local

# Supabase Local for Windows

ASP.NET Core 10 application foundation for a Windows installer and local Supabase manager.

## Runtime decision

The official Supabase local stack cannot currently run natively on Windows without a container engine. Its experimental native runtime supports Linux amd64/arm64 (Ubuntu 22.04 or later / glibc 2.35+) and Apple-silicon macOS.

A proposed Docker-free Windows implementation would use a dedicated Ubuntu WSL2 distribution for Supabase services and a Windows ASP.NET Core dashboard to manage them. WSL2 integration remains unimplemented pending the runtime decision. This repository does not yet contain a working Supabase installer.

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

No Git remote has been configured yet.

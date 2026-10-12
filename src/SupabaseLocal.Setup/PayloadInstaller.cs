using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using SupabaseLocal.Packaging;

namespace SupabaseLocal.Setup;

internal static class PayloadInstaller
{
    public static string Install(IProgress<string> progress)
    {
        using var payload = Assembly.GetExecutingAssembly().GetManifestResourceStream("SupabaseLocal.Payload.zip")
            ?? throw new InvalidOperationException("This developer build has no bundled services. Run scripts/build-installer.ps1 to create the installer.");
        var installBase = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SupabaseLocal");
        return InstallPayload(payload, installBase, progress);
    }
    internal static string InstallPayload(Stream payload, string installBase, IProgress<string> progress)
    {
        var version = Convert.ToHexString(SHA256.HashData(payload)).ToLowerInvariant()[..16];
        payload.Position = 0;
        using var archive = new ZipArchive(payload, ZipArchiveMode.Read, leaveOpen: true);
        var manifestEntry = archive.GetEntry("manifest.json") ?? throw new InvalidDataException("Payload has no checksum manifest.");
        using var manifestStream = manifestEntry.Open();
        var files = JsonSerializer.Deserialize<Dictionary<string, string>>(manifestStream) ?? throw new InvalidDataException("Invalid manifest.");
        var destination = Path.Combine(installBase, "versions", version);
        if (Directory.Exists(destination))
        {
            Verify(destination, files, progress);
            RuntimeLinks.Restore(Path.Combine(destination, "runtime"));
            return destination;
        }
        Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
        var staging = destination + ".staging-" + Guid.NewGuid().ToString("N");
        Directory.CreateDirectory(staging);
        try
        {
            int count = 0;
            foreach (var (name, expected) in files)
            {
                var target = SafePath(staging, name);
                var entry = archive.GetEntry(name) ?? throw new InvalidDataException("Missing payload file: " + name);
                Directory.CreateDirectory(Path.GetDirectoryName(target)!);
                using (var input = entry.Open())
                using (var output = File.Create(target)) input.CopyTo(output);
                using var check = File.OpenRead(target);
                if (!Convert.ToHexString(SHA256.HashData(check)).Equals(expected, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("Checksum mismatch: " + name);
                if (++count % 200 == 0) progress.Report($"Installing verified files: {count}/{files.Count}");
            }
            if (!File.Exists(Path.Combine(staging, "SupabaseLocal.exe"))) throw new InvalidDataException("Payload is missing the manager.");
            Directory.Move(staging, destination);
            try { RuntimeLinks.Restore(Path.Combine(destination, "runtime")); }
            catch { RuntimeLinks.Remove(Path.Combine(destination, "runtime")); Directory.Delete(destination, recursive: true); throw; }
            progress.Report("Native runtime installed. Starting services…");
            return destination;
        }
        finally
        {
            // Staging is a freshly-created child of the checked install directory.
            if (Directory.Exists(staging)) Directory.Delete(staging, recursive: true);
        }
    }
    private static string SafePath(string root, string name)
    {
        if (Path.IsPathRooted(name) || name.Contains(':')) throw new InvalidDataException("Unsafe payload path.");
        var target = Path.GetFullPath(Path.Combine(root, name));
        if (!target.StartsWith(Path.GetFullPath(root) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Payload path escapes the installation directory.");
        return target;
    }
    private static void Verify(string root, Dictionary<string, string> files, IProgress<string> progress)
    {
        progress.Report("Checking installed runtime…");
        foreach (var (name, expected) in files)
        {
            using var file = File.OpenRead(SafePath(root, name));
            if (!Convert.ToHexString(SHA256.HashData(file)).Equals(expected, StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("Installed runtime was modified: " + name);
        }
    }
}

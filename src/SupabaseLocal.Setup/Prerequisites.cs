using System.Diagnostics;
using System.Security.Cryptography;

namespace SupabaseLocal.Setup;

internal static class Prerequisites
{
    // This pinned package was downloaded from Microsoft's official endpoint and
    // its Authenticode signer verified as Microsoft Corporation at build time.
    private const string Url = "https://aka.ms/vs/17/release/vc_redist.x64.exe";
    private const string Hash = "CC0FF0EB1DC3F5188AE6300FAEF32BF5BEEBA4BDD6E8E445A9184072096B713B";
    public static async Task EnsureAsync(Action<string> report)
    {
        string[] dlls = ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll"];
        if (dlls.All(name => IsInstalled(Path.Combine(Environment.SystemDirectory, name)))) return;
        report("Installing the Microsoft C++ runtime required by PostgreSQL. Windows may request administrator approval.");
        var download = Path.Combine(Path.GetTempPath(), "SupabaseLocal-vcredist-" + Guid.NewGuid().ToString("N") + ".exe");
        try
        {
            using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(5) };
            using var response = await http.GetAsync(Url, HttpCompletionOption.ResponseHeadersRead);
            response.EnsureSuccessStatusCode();
            await using (var input = await response.Content.ReadAsStreamAsync())
            await using (var output = File.Create(download)) await input.CopyToAsync(output);
            await using (var file = File.OpenRead(download))
                if (!Convert.ToHexString(await SHA256.HashDataAsync(file)).Equals(Hash, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("Microsoft runtime download changed. Use a refreshed installer.");
            using var process = Process.Start(new ProcessStartInfo(download, "/install /quiet /norestart") { UseShellExecute = true, Verb = "runas" })
                ?? throw new InvalidOperationException("Could not start Microsoft's runtime installer.");
            await process.WaitForExitAsync();
            if (process.ExitCode == 3010) throw new InvalidOperationException("Windows needs a restart to finish installing the C++ runtime. Open this launcher again after restarting.");
            if (process.ExitCode is not (0 or 1638)) throw new InvalidOperationException("Microsoft runtime installation failed: " + process.ExitCode);
            if (!dlls.All(name => IsInstalled(Path.Combine(Environment.SystemDirectory, name)))) throw new InvalidOperationException("The required Microsoft runtime is still unavailable.");
        }
        finally { if (File.Exists(download)) File.Delete(download); }
    }
    private static bool IsInstalled(string file)
    {
        if (!File.Exists(file)) return false;
        var version = FileVersionInfo.GetVersionInfo(file);
        return version.FileMajorPart > 14 || version.FileMajorPart == 14 && version.FileMinorPart >= 44;
    }
}

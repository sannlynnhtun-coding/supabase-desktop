using System.IO.Compression;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using SupabaseLocal.Setup;

var root = Path.Combine(Path.GetTempPath(), "supabase-installer-checks-" + Guid.NewGuid().ToString("N"));
var progress = new Progress<string>();
Directory.CreateDirectory(root);
try
{
    using var good = Payload("SupabaseLocal.exe", "native payload", false);
    var installed = PayloadInstaller.InstallPayload(good, root, progress);
    if (File.ReadAllText(Path.Combine(installed, "SupabaseLocal.exe")) != "native payload") throw new Exception("Extraction failed");
    Directory.CreateDirectory(Path.Combine(root, "data"));
    File.WriteAllText(Path.Combine(root, "data", "sentinel"), "user data");
    good.Position = 0;
    if (PayloadInstaller.InstallPayload(good, root, progress) != installed) throw new Exception("Idempotent install changed version");
    File.WriteAllText(Path.Combine(installed, "SupabaseLocal.exe"), "tampered");
    good.Position = 0;
    ExpectFailure(() => PayloadInstaller.InstallPayload(good, root, progress), "modified installed payload");
    using var traversal = Payload("../../escape.exe", "bad path", false);
    ExpectFailure(() => PayloadInstaller.InstallPayload(traversal, root, progress), "path traversal");
    using var corrupt = Payload("SupabaseLocal.exe", "corrupt bytes", true);
    ExpectFailure(() => PayloadInstaller.InstallPayload(corrupt, root, progress), "checksum mismatch");
    if (Directory.GetDirectories(Path.Combine(root, "versions"), "*.staging-*").Length != 0) throw new Exception("Failed install leaked staging files");
    if (File.ReadAllText(Path.Combine(root, "data", "sentinel")) != "user data") throw new Exception("Install altered user data");
    if (OperatingSystem.IsWindows())
    {
        var linksRoot = Path.Combine(root, "runtime-links");
        Directory.CreateDirectory(Path.Combine(linksRoot, "packages", "example"));
        File.WriteAllText(Path.Combine(linksRoot, "packages", "example", "index.js"), "module.exports=42");
        File.WriteAllText(Path.Combine(linksRoot, "links.json"), "{\"node_modules/example\":{\"target\":\"packages/example\",\"directory\":true}}");
        SupabaseLocal.Packaging.RuntimeLinks.Restore(linksRoot);
        if (File.ReadAllText(Path.Combine(linksRoot, "node_modules", "example", "index.js")) != "module.exports=42") throw new Exception("Junction resolution failed");
        SupabaseLocal.Packaging.RuntimeLinks.Restore(linksRoot);
        SupabaseLocal.Packaging.RuntimeLinks.Remove(linksRoot);
        File.WriteAllText(Path.Combine(linksRoot, "links.json"), "{\"unsafe\":{\"target\":\"../../outside\",\"directory\":true}}");
        ExpectFailure(() => SupabaseLocal.Packaging.RuntimeLinks.Restore(linksRoot), "escaping runtime link");
        Console.WriteLine("PASS: native NTFS junction creation, repeated verification and escaping-link rejection.");
        var appRoot = Path.Combine(root, "app install with spaces");
        var version = Path.Combine(appRoot, "versions", "test-version");
        Directory.CreateDirectory(version);
        File.WriteAllText(Path.Combine(version, "SupabaseLocal.App.exe"), "test app");
        Directory.CreateDirectory(Path.Combine(appRoot, "data"));
        File.WriteAllText(Path.Combine(appRoot, "data", "sentinel"), "keep project");
        var scope = new InstallationScope(appRoot, Path.Combine(root, "test programs"), @"Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal.Test." + Guid.NewGuid().ToString("N"));
        try
        {
            WindowsInstallation.Register(version, scope);
            WindowsInstallation.Register(version, scope);
            if (!File.Exists(Path.Combine(scope.Programs, "Supabase Local.lnk"))) throw new Exception("Start menu link missing");
            using (var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(scope.RegistryPath))
                if (key?.GetValue("UninstallString") as string != $"\"{Path.Combine(version, "SupabaseLocal.App.exe")}\" --uninstall") throw new Exception("Uninstall command is incorrect");
            using (var held = new FileStream(Path.Combine(appRoot, "launcher.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None))
                ExpectIOException(() => WindowsInstallation.Remove(scope, false, progress), "running desktop app");
            var outside = Path.Combine(root, "outside uninstall target");
            Directory.CreateDirectory(outside);
            File.WriteAllText(Path.Combine(outside, "sentinel"), "do not delete");
            var runtime = Path.Combine(version, "runtime");
            Directory.CreateDirectory(Path.Combine(runtime, "target"));
            File.WriteAllText(Path.Combine(runtime, "links.json"), "{\"alias\":{\"target\":\"target\",\"directory\":true}}");
            SupabaseLocal.Packaging.RuntimeLinks.Restore(runtime);
            var junctionInfo = new System.Diagnostics.ProcessStartInfo("cmd.exe") { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
            foreach (var argument in new[] { "/d", "/c", "mklink", "/J", Path.Combine(version, "outside-link"), outside }) junctionInfo.ArgumentList.Add(argument);
            using (var junction = System.Diagnostics.Process.Start(junctionInfo)!)
            {
                junction.WaitForExit();
                if (junction.ExitCode != 0) throw new Exception("External junction fixture failed: " + junction.StandardError.ReadToEnd());
            }
            WindowsInstallation.Remove(scope, false, progress);
            if (Directory.Exists(Path.Combine(appRoot, "versions")) || Directory.Exists(scope.Programs)) throw new Exception("Uninstall left app files or shortcuts");
            if (File.ReadAllText(Path.Combine(appRoot, "data", "sentinel")) != "keep project") throw new Exception("Uninstall removed project data by default");
            using (var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(scope.RegistryPath))
                if (key is not null) throw new Exception("Uninstall registration was retained");
            Directory.CreateDirectory(version);
            File.WriteAllText(Path.Combine(version, "SupabaseLocal.App.exe"), "test app");
            WindowsInstallation.Register(version, scope);
            WindowsInstallation.Remove(scope, true, progress);
            if (Directory.Exists(appRoot)) throw new Exception("Explicit project-data removal failed");
            if (!File.Exists(Path.Combine(outside, "sentinel"))) throw new Exception("Uninstall escaped its root");
            Console.WriteLine("PASS: Start menu links, uninstall registry, repeat registration, running-app rejection, junction removal, default data preservation and explicit data removal.");
        }
        finally { Microsoft.Win32.Registry.CurrentUser.DeleteSubKeyTree(scope.RegistryPath, false); }
    }
    Console.WriteLine("PASS: extraction, repeat install, tamper detection, traversal rejection, corrupt-payload rollback and data preservation.");
}
finally { Directory.Delete(root, recursive: true); }

static void ExpectFailure(Action action, string label)
{
    try { action(); }
    catch (InvalidDataException) { return; }
    throw new Exception("Accepted " + label);
}
static void ExpectIOException(Action action, string label)
{
    try { action(); }
    catch (IOException) { return; }
    throw new Exception("Accepted " + label);
}
static MemoryStream Payload(string name, string value, bool corrupt)
{
    var stream = new MemoryStream();
    using (var archive = new ZipArchive(stream, ZipArchiveMode.Create, leaveOpen: true))
    {
        using (var writer = new StreamWriter(archive.CreateEntry(name).Open())) writer.Write(value);
        var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(corrupt ? "different content" : value)));
        using var manifest = archive.CreateEntry("manifest.json").Open();
        JsonSerializer.Serialize(manifest, new Dictionary<string, string> { [name] = hash });
    }
    stream.Position = 0;
    return stream;
}

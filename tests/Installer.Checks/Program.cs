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
    Console.WriteLine("PASS: extraction, repeat install, tamper detection, traversal rejection, corrupt-payload rollback and data preservation.");
}
finally { Directory.Delete(root, recursive: true); }

static void ExpectFailure(Action action, string label)
{
    try { action(); }
    catch (InvalidDataException) { return; }
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

using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;

if (args is ["--refresh-branding", var sourceBundle, var desktop, var uninstaller, var dashboard, var refreshedBundle])
{
    File.Copy(Path.GetFullPath(sourceBundle), Path.GetFullPath(refreshedBundle), overwrite: false);
    using var package = ZipFile.Open(Path.GetFullPath(refreshedBundle), ZipArchiveMode.Update);
    var manifestEntry = package.GetEntry("manifest.json") ?? throw new InvalidDataException("Missing manifest");
    Dictionary<string, string> files;
    using (var reader = new StreamReader(manifestEntry.Open()))
        files = JsonSerializer.Deserialize<Dictionary<string, string>>(reader.ReadToEnd())!;
    foreach (var (name, path) in new[] { ("SupabaseLocal.App.exe", desktop), ("SupabaseLocal.Uninstall.exe", uninstaller), ("wwwroot/index.html", dashboard) })
    {
        var existing = package.GetEntry(name) ?? throw new InvalidDataException("Missing entry: " + name);
        using (var old = existing.Open())
            if (!Convert.ToHexString(SHA256.HashData(old)).Equals(files[name], StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("Existing checksum mismatch: " + name);
        existing.Delete();
        using var input = File.OpenRead(Path.GetFullPath(path));
        files[name] = Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant();
        input.Position = 0;
        using var refreshedEntry = package.CreateEntry(name, CompressionLevel.Fastest).Open();
        input.CopyTo(refreshedEntry);
    }
    manifestEntry.Delete();
    using (var refreshedManifest = package.CreateEntry("manifest.json", CompressionLevel.Fastest).Open()) JsonSerializer.Serialize(refreshedManifest, files);
    Console.WriteLine($"Refreshed Rust desktop, Rust uninstaller and branded dashboard; retained {files.Count} manifest files.");
    return;
}

if (args is ["--verify", var archive])
{
    using var package = ZipFile.OpenRead(Path.GetFullPath(archive));
    using var reader = new StreamReader(package.GetEntry("manifest.json")!.Open());
    var files = JsonSerializer.Deserialize<Dictionary<string, string>>(reader.ReadToEnd())!;
    foreach (var (name, expected) in files)
    {
        using var entry = (package.GetEntry(name) ?? throw new InvalidDataException("Missing file: " + name)).Open();
        if (!Convert.ToHexString(SHA256.HashData(entry)).Equals(expected, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Hash mismatch: " + name);
    }
    Console.WriteLine($"Verified {files.Count} packaged files");
    return;
}
if (args.Length != 2) throw new ArgumentException("Usage: PayloadArchive <prepared-payload> <new-zip> | --verify <zip> | --refresh-branding <zip> <Rust-desktop> <Rust-uninstaller> <dashboard> <new-zip>");
var root = Path.GetFullPath(args[0]);
var manifest = JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(Path.Combine(root, "manifest.json"))) ?? throw new InvalidDataException("Invalid manifest");
using var output = new FileStream(Path.GetFullPath(args[1]), FileMode.CreateNew);
using var zip = new ZipArchive(output, ZipArchiveMode.Create);
var buffer = new byte[65536];
var count = 0;
foreach (var (name, expected) in manifest)
{
    if (Path.IsPathRooted(name) || name.Contains(':')) throw new InvalidDataException("Unsafe archive filename");
    var path = Path.GetFullPath(Path.Combine(root, name));
    if (!path.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("Archive path escapes payload");
    // Archive the manifest's regular files, never traverse restored pnpm junctions.
    using var source = File.OpenRead(path);
    using var target = zip.CreateEntry(name.Replace('\\', '/'), CompressionLevel.Fastest).Open();
    using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
    int read;
    while ((read = source.Read(buffer)) != 0) { hash.AppendData(buffer, 0, read); target.Write(buffer, 0, read); }
    if (!Convert.ToHexString(hash.GetHashAndReset()).Equals(expected, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("Payload changed: " + name);
    if (++count % 10000 == 0) Console.WriteLine($"Archived and verified {count}/{manifest.Count} files");
}
zip.CreateEntryFromFile(Path.Combine(root, "manifest.json"), "manifest.json", CompressionLevel.Fastest);
Console.WriteLine($"Completed {count} verified files");

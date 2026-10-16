using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;

if (args is ["--replace-desktop", var sourceArchive, var rustDesktop, var outputArchive])
{
    File.Copy(Path.GetFullPath(sourceArchive), Path.GetFullPath(outputArchive), overwrite: false);
    using var package = ZipFile.Open(Path.GetFullPath(outputArchive), ZipArchiveMode.Update);
    var manifestEntry = package.GetEntry("manifest.json") ?? throw new InvalidDataException("Missing manifest");
    Dictionary<string, string> files;
    using (var reader = new StreamReader(manifestEntry.Open()))
        files = JsonSerializer.Deserialize<Dictionary<string, string>>(reader.ReadToEnd())!;
    const string desktopName = "SupabaseLocal.App.exe";
    const string uninstallName = "SupabaseLocal.Uninstall.exe";
    var desktopEntry = package.GetEntry(desktopName) ?? throw new InvalidDataException("Missing desktop app");
    if (package.GetEntry(uninstallName) is null)
    {
        using var oldDesktop = new MemoryStream();
        using (var input = desktopEntry.Open()) input.CopyTo(oldDesktop);
        var bytes = oldDesktop.ToArray();
        var hash = Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant();
        if (!hash.Equals(files[desktopName], StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("Existing desktop checksum mismatch");
        using (var uninstallOutput = package.CreateEntry(uninstallName, CompressionLevel.Fastest).Open()) uninstallOutput.Write(bytes);
        files[uninstallName] = hash;
    }
    desktopEntry.Delete();
    using (var input = File.OpenRead(Path.GetFullPath(rustDesktop)))
    {
        files[desktopName] = Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant();
        input.Position = 0;
        using var desktopOutput = package.CreateEntry(desktopName, CompressionLevel.Fastest).Open();
        input.CopyTo(desktopOutput);
    }
    manifestEntry.Delete();
    using (var manifestOutput = package.CreateEntry("manifest.json", CompressionLevel.Fastest).Open()) JsonSerializer.Serialize(manifestOutput, files);
    Console.WriteLine($"Updated verified desktop entries; retained {files.Count} manifest files.");
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
if (args.Length != 2) throw new ArgumentException("Usage: PayloadArchive <prepared-payload> <new-zip> | --verify <zip>");
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

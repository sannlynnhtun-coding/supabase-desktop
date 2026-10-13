using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;

if (args.Length != 2) throw new ArgumentException("Usage: PayloadArchive <prepared-payload> <new-zip>");
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

using System.Buffers.Binary;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;

namespace SupabaseLocal.Packaging;

internal static class RuntimeLinks
{
    private sealed record Link(string Target, bool Directory);
    public static void Restore(string root)
    {
        var metadata = Path.Combine(root, "links.json");
        if (!File.Exists(metadata)) return;
        var links = JsonSerializer.Deserialize<Dictionary<string, Link>>(File.ReadAllText(metadata), new JsonSerializerOptions { PropertyNameCaseInsensitive = true }) ?? throw new InvalidDataException("Invalid runtime links.");
        foreach (var (relative, link) in links)
        {
            var alias = Within(root, relative);
            var target = Within(root, link.Target);
            if (alias == target) throw new InvalidDataException("Runtime link targets itself.");
            Directory.CreateDirectory(Path.GetDirectoryName(alias)!);
            if (link.Directory)
            {
                if (!Directory.Exists(target)) throw new InvalidDataException("Missing runtime package: " + link.Target);
                if (Directory.Exists(alias))
                {
                    var existing = new DirectoryInfo(alias).LinkTarget;
                    if (existing is null || !Path.GetFullPath(existing, Path.GetDirectoryName(alias)!).Equals(target, StringComparison.OrdinalIgnoreCase))
                        throw new InvalidDataException("Runtime link was modified: " + relative);
                }
                else Junction(alias, target);
            }
            else
            {
                if (!File.Exists(target)) throw new InvalidDataException("Missing runtime file: " + link.Target);
                if (File.Exists(alias))
                {
                    using var left = File.OpenRead(alias);
                    using var right = File.OpenRead(target);
                    if (!System.Security.Cryptography.SHA256.HashData(left).AsSpan().SequenceEqual(System.Security.Cryptography.SHA256.HashData(right)))
                        throw new InvalidDataException("Runtime file link was modified: " + relative);
                }
                else if (!CreateHardLink(alias, target, IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error());
            }
        }
    }
    public static void Remove(string root)
    {
        var metadata = Path.Combine(root, "links.json");
        if (!File.Exists(metadata)) return;
        var links = JsonSerializer.Deserialize<Dictionary<string, Link>>(File.ReadAllText(metadata), new JsonSerializerOptions { PropertyNameCaseInsensitive = true }) ?? throw new InvalidDataException("Invalid runtime links.");
        foreach (var (relative, link) in links.Reverse())
        {
            var alias = Within(root, relative);
            if (link.Directory)
            {
                // Detach junctions before deleting their targets; a recursive
                // Windows delete can fail on an already-dangling mount point.
                if (new DirectoryInfo(alias).LinkTarget is not null) Directory.Delete(alias);
            }
            else if (File.Exists(alias)) File.Delete(alias);
        }
    }
    private static string Within(string root, string relative)
    {
        if (Path.IsPathRooted(relative) || relative.Contains(':')) throw new InvalidDataException("Unsafe runtime link.");
        var path = Path.GetFullPath(Path.Combine(root, relative));
        if (!path.StartsWith(Path.GetFullPath(root) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("Runtime link escapes installation directory.");
        return path;
    }
    private static void Junction(string alias, string target)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Runtime junctions require Windows/NTFS.");
        Directory.CreateDirectory(alias);
        using var handle = CreateFile(alias, 0x40000000, 7, IntPtr.Zero, 3, 0x02200000, IntPtr.Zero);
        if (handle.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
        var substitute = Encoding.Unicode.GetBytes("\\??\\" + target);
        var print = Encoding.Unicode.GetBytes(target);
        var buffer = new byte[16 + substitute.Length + 2 + print.Length + 2];
        BinaryPrimitives.WriteUInt32LittleEndian(buffer.AsSpan(0), 0xA0000003); // mount point tag
        BinaryPrimitives.WriteUInt16LittleEndian(buffer.AsSpan(4), checked((ushort)(buffer.Length - 8)));
        BinaryPrimitives.WriteUInt16LittleEndian(buffer.AsSpan(10), checked((ushort)substitute.Length));
        BinaryPrimitives.WriteUInt16LittleEndian(buffer.AsSpan(12), checked((ushort)(substitute.Length + 2)));
        BinaryPrimitives.WriteUInt16LittleEndian(buffer.AsSpan(14), checked((ushort)print.Length));
        substitute.CopyTo(buffer, 16);
        print.CopyTo(buffer, 16 + substitute.Length + 2);
        if (!DeviceIoControl(handle, 0x000900A4, buffer, buffer.Length, IntPtr.Zero, 0, out _, IntPtr.Zero))
            throw new Win32Exception(Marshal.GetLastWin32Error());
    }
    [DllImport("kernel32.dll", EntryPoint = "CreateFileW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateFile(string name, uint access, uint share, IntPtr security, uint creation, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeviceIoControl(SafeFileHandle handle, uint code, byte[] input, int inputSize, IntPtr output, int outputSize, out int returned, IntPtr overlapped);
    [DllImport("kernel32.dll", EntryPoint = "CreateHardLinkW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CreateHardLink(string newName, string existingName, IntPtr security);
}

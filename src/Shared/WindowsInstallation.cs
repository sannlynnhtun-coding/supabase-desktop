using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Microsoft.Win32;

namespace SupabaseLocal.Setup;

[SupportedOSPlatform("windows")]
internal sealed record InstallationScope(string Root, string Programs, string RegistryPath)
{
    public static InstallationScope Current => new(
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SupabaseLocal"),
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs), "Supabase Local"),
        @"Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal");
}

[SupportedOSPlatform("windows")]
internal static class WindowsInstallation
{
    private const string Owner = "SupabaseLocal.NativeWindows.v1";
    public static void Register(string version, InstallationScope scope)
    {
        var root = Path.GetFullPath(scope.Root);
        var executable = Path.Combine(Path.GetFullPath(version), "SupabaseLocal.App.exe");
        if (!executable.StartsWith(Path.Combine(root, "versions") + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase) || !File.Exists(executable))
            throw new InvalidDataException("Installed app must be inside the verified version directory.");
        Directory.CreateDirectory(scope.Programs);
        CreateShortcut(Path.Combine(scope.Programs, "Supabase Local.lnk"), executable, "", "Start Supabase Local and open Studio");
        CreateShortcut(Path.Combine(scope.Programs, "Uninstall Supabase Local.lnk"), executable, "--uninstall", "Uninstall Supabase Local");
        using var key = Registry.CurrentUser.CreateSubKey(scope.RegistryPath);
        key.SetValue("InstallerOwner", Owner);
        key.SetValue("DisplayName", "Supabase Local");
        key.SetValue("DisplayVersion", "0.2.0");
        key.SetValue("Publisher", "Supabase Local community port");
        key.SetValue("InstallLocation", root);
        key.SetValue("DisplayIcon", executable);
        key.SetValue("UninstallString", $"\"{executable}\" --uninstall");
        key.SetValue("URLInfoAbout", "https://github.com/sannlynnhtun-coding/supabase-local");
        key.SetValue("NoModify", 1, RegistryValueKind.DWord);
        key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
    }

    public static void Remove(InstallationScope scope, bool removeData, IProgress<string> progress)
    {
        var root = Path.GetFullPath(scope.Root);
        if (root == Path.GetPathRoot(root)) throw new InvalidDataException("Cannot uninstall a drive root.");
        CheckParents(root);
        using (var key = Registry.CurrentUser.OpenSubKey(scope.RegistryPath))
        {
            if (key?.GetValue("InstallerOwner") as string != Owner || !string.Equals(key.GetValue("InstallLocation") as string, root, StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("Uninstall registration does not belong to this installation.");
        }
        Directory.CreateDirectory(root);
        using (var launcherLock = new FileStream(Path.Combine(root, "launcher.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None))
        {
            var data = Path.Combine(root, "data");
            CheckParents(data);
            // Also reject a manager launched without the desktop app.
            using (var managerLock = Directory.Exists(data) ? new FileStream(Path.Combine(data, "manager.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None) : null)
            {
                progress.Report("Removing application files…");
                DeleteTree(Path.Combine(root, "versions"), root);
            }
            if (removeData) { progress.Report("Removing project data…"); DeleteTree(data, root); }
            progress.Report("Removing Start menu and Installed apps entries…");
            foreach (var name in new[] { "Supabase Local.lnk", "Uninstall Supabase Local.lnk" })
                File.Delete(Path.Combine(scope.Programs, name));
            if (Directory.Exists(scope.Programs) && !Directory.EnumerateFileSystemEntries(scope.Programs).Any()) Directory.Delete(scope.Programs);
            Registry.CurrentUser.DeleteSubKeyTree(scope.RegistryPath, throwOnMissingSubKey: false);
        }
        File.Delete(Path.Combine(root, "launcher.lock"));
        if (!Directory.EnumerateFileSystemEntries(root).Any()) Directory.Delete(root);
    }

    internal static void DeleteTree(string path, string root)
    {
        path = Path.GetFullPath(path);
        if (!path.StartsWith(Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Removal target escapes installation.");
        var directory = new DirectoryInfo(path);
        if (!directory.Exists && directory.LinkTarget is null) return;
        // Detach junctions/symlinks without traversing their targets, including broken links.
        if ((directory.Attributes & FileAttributes.ReparsePoint) != 0) { Directory.Delete(path); return; }
        var entries = directory.GetFileSystemInfos();
        foreach (var entry in entries.Where(e => (e.Attributes & FileAttributes.ReparsePoint) != 0))
        {
            if ((entry.Attributes & FileAttributes.Directory) != 0) Directory.Delete(entry.FullName);
            else File.Delete(entry.FullName);
        }
        foreach (var entry in entries.Where(e => (e.Attributes & FileAttributes.ReparsePoint) == 0))
        {
            if (entry is DirectoryInfo) DeleteTree(entry.FullName, root);
            else { entry.Attributes &= ~FileAttributes.ReadOnly; File.Delete(entry.FullName); }
        }
        Directory.Delete(path);
    }

    private static void CheckParents(string path)
    {
        for (var directory = new DirectoryInfo(path); directory is not null; directory = directory.Parent)
            if (directory.Exists && (directory.Attributes & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("Installation paths cannot pass through junctions or symlinks.");
    }

    private static void CreateShortcut(string path, string executable, string arguments, string description)
    {
        dynamic shell = Activator.CreateInstance(Type.GetTypeFromProgID("WScript.Shell")!)!;
        object? shortcutObject = null;
        try
        {
            dynamic shortcut = shortcutObject = shell.CreateShortcut(path);
            shortcut.TargetPath = executable;
            shortcut.WorkingDirectory = Path.GetDirectoryName(executable);
            shortcut.Arguments = arguments;
            shortcut.Description = description;
            shortcut.IconLocation = executable + ",0";
            shortcut.Save();
        }
        finally
        {
            if (shortcutObject is not null) Marshal.FinalReleaseComObject(shortcutObject);
            Marshal.FinalReleaseComObject(shell);
        }
    }
}

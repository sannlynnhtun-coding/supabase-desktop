using System.Diagnostics;
using SupabaseLocal.Setup;

internal sealed class UninstallWindow : Form
{
    private readonly Label status = new() { Dock = DockStyle.Fill, Padding = new Padding(20), Text = "Remove Supabase Desktop, its Start menu shortcuts and its Windows app entry. Close Supabase Desktop before uninstalling. Project data is kept unless you select the option below." };
    private readonly CheckBox purge = new() { Dock = DockStyle.Bottom, Height = 45, Text = "Also permanently delete my local project data", Padding = new Padding(20, 0, 0, 0) };
    private readonly Button action = new() { Dock = DockStyle.Bottom, Height = 45, Text = "Uninstall" };
    private bool busy;

    public UninstallWindow(int? parent = null, bool removeData = false)
    {
        Text = "Uninstall Supabase Desktop";
        Icon = AppBranding.WindowIcon();
        ClientSize = new Size(580, 350);
        StartPosition = FormStartPosition.CenterScreen;
        Controls.Add(status); Controls.Add(purge); Controls.Add(action);
        Controls.Add(AppBranding.Header());
        FormClosing += (_, e) => e.Cancel = busy;
        if (parent is not null)
        {
            purge.Visible = false; action.Visible = false;
            Shown += async (_, _) => await RemoveAsync(parent.Value, removeData);
        }
        else action.Click += (_, _) => StartWorker();
    }

    private void StartWorker()
    {
        if (purge.Checked && MessageBox.Show(this, "Permanently delete the local database, uploaded files, secrets and snippets? This cannot be undone.", "Delete project data", MessageBoxButtons.YesNo, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2) != DialogResult.Yes) return;
        try
        {
            var scope = InstallationScope.Current;
            using (var appLock = new FileStream(Path.Combine(scope.Root, "launcher.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None)) { }
            var temporary = Path.Combine(Path.GetTempPath(), "SupabaseLocal-Uninstall-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var executable = Path.Combine(temporary, "SupabaseLocal.App.exe");
            File.Copy(Environment.ProcessPath!, executable);
            var info = new ProcessStartInfo(executable) { UseShellExecute = false, WorkingDirectory = temporary };
            info.ArgumentList.Add("--uninstall-worker");
            info.ArgumentList.Add(Environment.ProcessId.ToString());
            info.ArgumentList.Add(purge.Checked.ToString());
            using var worker = Process.Start(info) ?? throw new InvalidOperationException("Could not start the uninstaller.");
            Close();
        }
        catch (IOException) { status.Text = "Close Supabase Desktop and its running services, then click Uninstall again."; }
        catch (Exception ex) { status.Text = "Uninstall could not start: " + ex.Message; }
    }

    private async Task RemoveAsync(int parent, bool removeData)
    {
        busy = true;
        var temporary = AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar);
        try
        {
            var expected = Path.GetFullPath(Path.GetTempPath()).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar + "SupabaseLocal-Uninstall-";
            if (!temporary.StartsWith(expected, StringComparison.OrdinalIgnoreCase) || temporary[expected.Length..].Length != 32 || !Guid.TryParseExact(temporary[expected.Length..], "N", out _))
                throw new InvalidDataException("Uninstall worker must run from its private temporary directory.");
            try
            {
                using var original = Process.GetProcessById(parent);
                using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
                await original.WaitForExitAsync(timeout.Token);
            }
            catch (ArgumentException) { }
            status.Text = removeData ? "Removing application and project data…" : "Removing application. Your project data will be kept…";
            var progress = new Progress<string>(text => status.Text = text);
            await Task.Run(() => WindowsInstallation.Remove(InstallationScope.Current, removeData, progress));
            status.Text = removeData ? "Supabase Desktop and its project data have been removed." : "Supabase Desktop has been removed. Your project data is kept in:\n" + Path.Combine(InstallationScope.Current.Root, "data");
        }
        catch (IOException ex) { status.Text = "Close Supabase Desktop and its services, then try uninstalling again.\n" + ex.Message; Environment.ExitCode = 1; }
        catch (Exception ex) { status.Text = "Uninstall could not finish: " + ex.Message; Environment.ExitCode = 1; }
        finally
        {
            busy = false;
            action.Text = "Close"; action.Visible = true;
            action.Click += (_, _) => Close();
            FormClosed += (_, _) => ScheduleTemporaryCleanup(temporary);
        }
    }

    private static void ScheduleTemporaryCleanup(string directory)
    {
        // One PowerShell process removes only this worker's checked, private temp folder
        // after the worker exits; no shell-built filesystem commands are used.
        var prefix = Path.GetFullPath(Path.GetTempPath()).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar + "SupabaseLocal-Uninstall-";
        if (!directory.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) || !Guid.TryParseExact(directory[prefix.Length..], "N", out _)) return;
        try
        {
            var script = Path.Combine(directory, "cleanup.ps1");
            File.WriteAllText(script, "param([int]$WorkerId,[string]$Folder)\nWait-Process -Id $WorkerId -ErrorAction SilentlyContinue\n$full=[IO.Path]::GetFullPath($Folder)\n$prefix=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\\')+'\\SupabaseLocal-Uninstall-'\nif (!$full.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe cleanup path' }\n$guid=[Guid]::Empty\nif (![Guid]::TryParseExact($full.Substring($prefix.Length),'N',[ref]$guid)) { throw 'Invalid cleanup directory' }\nRemove-Item -LiteralPath $full -Recurse -Force\n");
            var info = new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "WindowsPowerShell/v1.0/powershell.exe")) { UseShellExecute = false, CreateNoWindow = true };
            foreach (var argument in new[] { "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", script, "-WorkerId", Environment.ProcessId.ToString(), "-Folder", directory }) info.ArgumentList.Add(argument);
            Process.Start(info)?.Dispose();
        }
        catch (IOException) { }
        catch (System.ComponentModel.Win32Exception) { }
    }
}

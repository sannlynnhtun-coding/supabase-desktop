using System.Diagnostics;
using System.Net.Http.Json;
using System.Security.Cryptography;

namespace SupabaseLocal.Setup;

internal sealed class SetupWindow : Form
{
    private readonly Label message = new() { Dock = DockStyle.Fill, Padding = new Padding(24), Text = "Preparing native Windows runtime…", Font = new Font("Segoe UI", 12) };
    private readonly Button open = new() { Dock = DockStyle.Bottom, Height = 48, Text = "Open Supabase Studio", Enabled = false };
    private readonly HttpClient http = new() { Timeout = TimeSpan.FromSeconds(5) };
    private readonly string token = Convert.ToHexString(RandomNumberGenerator.GetBytes(32));
    private Process? manager;
    private bool closing;
    private bool installing = true;
    private readonly bool installed;
    private FileStream? launcherLock;
    private readonly string? verificationData;
    public SetupWindow(bool installed = false, string? verificationData = null)
    {
        this.installed = installed;
        this.verificationData = verificationData;
        if (verificationData is not null) ShowInTaskbar = false;
        Text = "Supabase Local — Native Windows";
        ClientSize = new Size(580, 230);
        StartPosition = FormStartPosition.CenterScreen;
        Controls.Add(message);
        Controls.Add(open);
        open.Click += (_, _) => Browse("http://127.0.0.1:54323");
        Shown += async (_, _) => await InstallAndStartAsync();
        FormClosing += async (_, e) =>
        {
            if (installing) { e.Cancel = true; message.Text = "Finishing installation before closing…"; return; }
            if (closing || manager is null || manager.HasExited) return;
            e.Cancel = true;
            closing = true;
            open.Enabled = false;
            message.Text = "Stopping private Supabase processes…";
            try
            {
                using var request = new HttpRequestMessage(HttpMethod.Post, "http://127.0.0.1:54320/api/shutdown");
                request.Headers.Add("X-Launcher-Token", token);
                using var response = await http.SendAsync(request);
                response.EnsureSuccessStatusCode();
                using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
                await manager.WaitForExitAsync(timeout.Token);
            }
            catch (Exception ex)
            {
                message.Text = "Graceful shutdown failed: " + ex.Message + "\nKeep this window open and check the manager log.";
                closing = false;
                return;
            }
            Close();
        };
    }
    private async Task InstallAndStartAsync()
    {
        try
        {
            await Prerequisites.EnsureAsync(text => message.Text = text);
            Directory.CreateDirectory(InstallationScope.Current.Root);
            launcherLock = new FileStream(Path.Combine(InstallationScope.Current.Root, "launcher.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
            var directory = installed ? AppContext.BaseDirectory : await Task.Run(() => PayloadInstaller.Install(new Progress<string>(text => BeginInvoke(() => message.Text = text))));
            if (!installed) WindowsInstallation.Register(directory, InstallationScope.Current);
            var info = new ProcessStartInfo(Path.Combine(directory, "SupabaseLocal.exe")) { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = directory };
            info.Environment["LauncherToken"] = token;
            info.Environment["RuntimeRoot"] = Path.Combine(directory, "runtime");
            info.Environment["DataRoot"] = verificationData ?? Path.Combine(InstallationScope.Current.Root, "data");
            manager = Process.Start(info) ?? throw new InvalidOperationException("Could not start the manager.");
            for (int attempt = 0; attempt < 300; attempt++)
            {
                if (manager.HasExited) throw new InvalidOperationException("Manager exited. Check for conflicting ports or another open launcher.");
                try
                {
                    var status = await http.GetFromJsonAsync<Status>("http://127.0.0.1:54320/api/status");
                    if (status is not null)
                    {
                        message.Text = status.Message;
                        if (status.State == "failed") throw new InvalidOperationException(status.Message);
                        if (status.StudioUrl is not null)
                        {
                            open.Enabled = true;
                            if (verificationData is null) Browse(status.StudioUrl);
                            else
                            {
                                await File.WriteAllTextAsync(Path.Combine(verificationData, "launcher-verification.json"), System.Text.Json.JsonSerializer.Serialize(status));
                                BeginInvoke(() => Close());
                            }
                            return;
                        }
                    }
                }
                catch (HttpRequestException) { }
                catch (TaskCanceledException) { }
                await Task.Delay(1000);
            }
            throw new TimeoutException("Services did not start within five minutes. Check logs in %LOCALAPPDATA%\\SupabaseLocal\\data.");
        }
        catch (Exception ex)
        {
            message.Text = "Setup could not finish:\n" + ex.Message;
            if (verificationData is not null) { Environment.ExitCode = 1; BeginInvoke(() => Close()); }
        }
        finally { installing = false; }
    }
    private static void Browse(string url) => Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
    private sealed record Status(string State, string Message, string? StudioUrl, string[] Available);
    protected override void Dispose(bool disposing)
    {
        if (disposing) { manager?.Dispose(); http.Dispose(); launcherLock?.Dispose(); }
        base.Dispose(disposing);
    }
}

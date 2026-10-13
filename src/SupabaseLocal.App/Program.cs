using SupabaseLocal.Setup;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        if (args is ["--uninstall-worker", var parent, var purge] && int.TryParse(parent, out var pid) && bool.TryParse(purge, out var removeData))
            Application.Run(new UninstallWindow(pid, removeData));
        else if (args is ["--uninstall"])
            Application.Run(new UninstallWindow());
        else if (args is ["--register"])
            WindowsInstallation.Register(AppContext.BaseDirectory, InstallationScope.Current);
        else if (args is ["--verify-launch", var data])
            Application.Run(new SetupWindow(installed: true, verificationData: Path.GetFullPath(data)));
        else if (args is ["--branding-preview", var output])
        {
            Directory.CreateDirectory(Path.GetFullPath(output));
            using var launcher = new SetupWindow(installed: true, brandingPreview: true);
            using var uninstall = new UninstallWindow();
            foreach (var (form, name) in new[] { ((Form)launcher, "launcher.png"), ((Form)uninstall, "uninstall.png") })
            {
                form.Show();
                foreach (Control control in form.Controls) control.CreateControl();
                form.PerformLayout();
                Application.DoEvents();
                using var bitmap = new Bitmap(form.Width, form.Height);
                form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, form.Size));
                bitmap.Save(Path.Combine(output, name), System.Drawing.Imaging.ImageFormat.Png);
            }
        }
        else if (args.Length == 0)
            Application.Run(new SetupWindow(installed: true));
        else Environment.ExitCode = 2;
    }
}

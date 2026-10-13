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
        else if (args.Length == 0)
            Application.Run(new SetupWindow(installed: true));
        else Environment.ExitCode = 2;
    }
}

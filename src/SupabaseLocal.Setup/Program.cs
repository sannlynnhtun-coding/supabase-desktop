namespace SupabaseLocal.Setup;

static class Program
{
    /// <summary>
    ///  The main entry point for the application.
    /// </summary>
    [STAThread]
    static void Main(string[] args)
    {
        // Package verification mode exercises the same embedded-payload installer
        // without showing UI or starting services on an automated build machine.
        if (args is ["--verify-install", var directory])
        {
            try
            {
                var root = Path.GetFullPath(directory);
                using var stream = System.Reflection.Assembly.GetExecutingAssembly().GetManifestResourceStream("SupabaseLocal.Payload.zip") ?? throw new InvalidOperationException("No payload bundled.");
                var version = PayloadInstaller.InstallPayload(stream, root, new Progress<string>());
                File.WriteAllText(Path.Combine(root, "verification-result.json"), System.Text.Json.JsonSerializer.Serialize(new { installed = version }));
            }
            catch (Exception ex) { Environment.ExitCode = 1; File.WriteAllText(Path.GetFullPath(directory) + ".error.txt", ex.ToString()); }
            return;
        }
        // To customize application configuration such as set high DPI settings or default font,
        // see https://aka.ms/applicationconfiguration.
        ApplicationConfiguration.Initialize();
        Application.Run(new SetupWindow());
    }    
}

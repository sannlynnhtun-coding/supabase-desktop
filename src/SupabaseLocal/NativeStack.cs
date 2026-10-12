using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using System.Security.AccessControl;
using System.Security.Principal;

namespace SupabaseLocal;

public sealed record StackStatus(string State, string Message, string? StudioUrl, string[] Available, string[] Unavailable);
public sealed record LocalSecrets(string DatabasePassword, string JwtSecret, string MetaCryptoKey);

/// <summary>Owns only the private Windows processes started by this instance.</summary>
public sealed class NativeStack(IConfiguration configuration, ILogger<NativeStack> logger, ApiKeys keys) : BackgroundService
{
    private readonly List<Process> children = [];
    private readonly HttpClient http = new() { Timeout = TimeSpan.FromSeconds(5) };
    private readonly string root = Path.GetFullPath(configuration["RuntimeRoot"] ?? Path.Combine(AppContext.BaseDirectory, "runtime"));
    private readonly string data = Path.GetFullPath(configuration["DataRoot"] ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SupabaseLocal", "data"));
    private StackStatus status = new("starting", "Checking native runtime files", null, [], ["Auth", "Storage", "Realtime", "Edge Functions", "Supabase extensions"]);
    public StackStatus Status => Volatile.Read(ref status);
    private void Update(string state, string message, string? studio = null, params string[] available) =>
        Volatile.Write(ref status, new(state, message, studio, available, Status.Unavailable));

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        bool postgresOwned = false;
        FileStream? instanceLock = null;
        try
        {
            if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("This port requires native Windows.");
            Packaging.RuntimeLinks.Restore(root);
            string[] required = ["postgres/bin/initdb.exe", "postgres/bin/pg_ctl.exe", "node/node.exe", "pgmeta/dist/server/server.js", "studio/server.js"];
            var missing = required.Where(file => !File.Exists(Path.Combine(root, file))).ToArray();
            if (missing.Length > 0) throw new InvalidOperationException("Runtime payload is incomplete: " + string.Join(", ", missing));
            Directory.CreateDirectory(data);
            var access = new DirectorySecurity();
            access.SetAccessRuleProtection(isProtected: true, preserveInheritance: false);
            access.AddAccessRule(new FileSystemAccessRule(WindowsIdentity.GetCurrent().User!, FileSystemRights.FullControl, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
            new DirectoryInfo(data).SetAccessControl(access);
            instanceLock = new FileStream(Path.Combine(data, "manager.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
            var secretsFile = Path.Combine(data, "secrets.json");
            LocalSecrets secrets;
            if (File.Exists(secretsFile))
                secrets = JsonSerializer.Deserialize<LocalSecrets>(await File.ReadAllTextAsync(secretsFile, stoppingToken)) ?? throw new InvalidDataException("Invalid secrets file.");
            else
            {
                secrets = new(Convert.ToHexString(RandomNumberGenerator.GetBytes(24)), Convert.ToHexString(RandomNumberGenerator.GetBytes(32)), Convert.ToHexString(RandomNumberGenerator.GetBytes(32)));
                await File.WriteAllTextAsync(secretsFile, JsonSerializer.Serialize(secrets), stoppingToken);
            }
            keys.Initialize(secrets.JwtSecret);
            var pgData = Path.Combine(data, "postgres");
            if (!File.Exists(Path.Combine(pgData, "PG_VERSION")))
            {
                Update("starting", "Initializing private PostgreSQL database");
                var passwordFile = Path.Combine(data, "initdb-password.txt");
                try
                {
                    await File.WriteAllTextAsync(passwordFile, secrets.DatabasePassword, stoppingToken);
                    await RunAsync("postgres/bin/initdb.exe", ["-D", pgData, "-U", "postgres", "--pwfile", passwordFile, "--auth-host=scram-sha-256", "--auth-local=scram-sha-256", "--encoding=UTF8", "--locale=C"], null, stoppingToken);
                }
                finally { File.Delete(passwordFile); }
                await File.AppendAllTextAsync(Path.Combine(pgData, "postgresql.conf"), "\nlisten_addresses='127.0.0.1'\nport=54322\nwal_level=logical\n", stoppingToken);
            }
            Update("starting", "Starting PostgreSQL on 127.0.0.1:54322");
            // pg_ctl starts a detached server. Stop it explicitly during shutdown.
            await RunAsync("postgres/bin/pg_ctl.exe", ["-D", pgData, "-l", Path.Combine(data, "postgres.log"), "-w", "-t", "60", "start"], null, CancellationToken.None);
            postgresOwned = true;
            var pgEnv = new Dictionary<string, string> { ["PGPASSWORD"] = secrets.DatabasePassword };
            await RunAsync("postgres/bin/psql.exe", ["-h", "127.0.0.1", "-p", "54322", "-U", "postgres", "-d", "postgres", "-v", "ON_ERROR_STOP=1", "-c", "SELECT 1"], pgEnv, stoppingToken);
            pgEnv["SUPABASE_LOCAL_DATABASE_PASSWORD"] = secrets.DatabasePassword;
            await RunAsync("postgres/bin/psql.exe", ["-h", "127.0.0.1", "-p", "54322", "-U", "postgres", "-d", "postgres", "-v", "ON_ERROR_STOP=1", "-f", Path.Combine(AppContext.BaseDirectory, "bootstrap.sql")], pgEnv, stoppingToken);
            var common = new Dictionary<string, string>
            {
                ["PG_META_HOST"] = "127.0.0.1", ["PG_META_PORT"] = "54324",
                ["PG_META_DB_HOST"] = "127.0.0.1", ["PG_META_DB_PORT"] = "54322",
                ["PG_META_DB_USER"] = "postgres", ["PG_META_DB_NAME"] = "postgres",
                ["PG_META_DB_PASSWORD"] = secrets.DatabasePassword, ["CRYPTO_KEY"] = secrets.MetaCryptoKey,
            };
            Start("node/node.exe", [Path.Combine(root, "pgmeta/dist/server/server.js")], Path.Combine(root, "pgmeta"), common, "pgmeta");
            await WaitHttpAsync("http://127.0.0.1:54324/health", stoppingToken);
            var available = new List<string> { "PostgreSQL", "postgres-meta", "Studio" };
            if (File.Exists(Path.Combine(root, "postgrest/postgrest.exe")))
            {
                Start("postgrest/postgrest.exe", [], Path.Combine(root, "postgrest"), new()
                {
                    ["PGRST_DB_URI"] = $"postgresql://authenticator:{secrets.DatabasePassword}@127.0.0.1:54322/postgres",
                    ["PGRST_DB_SCHEMAS"] = "public", ["PGRST_DB_ANON_ROLE"] = "anon", ["PGRST_JWT_SECRET"] = secrets.JwtSecret,
                    ["PGRST_SERVER_HOST"] = "127.0.0.1", ["PGRST_SERVER_PORT"] = "54326",
                }, "postgrest");
                await WaitHttpAsync("http://127.0.0.1:54326/", stoppingToken);
                available.Add("PostgREST");
            }
            if (File.Exists(Path.Combine(root, "auth/auth.exe")))
            {
                Start("auth/auth.exe", [], Path.Combine(root, "auth"), new()
                {
                    ["GOTRUE_API_HOST"] = "127.0.0.1", ["GOTRUE_API_PORT"] = "54328",
                    ["API_EXTERNAL_URL"] = "http://127.0.0.1:54321/auth/v1", ["GOTRUE_SITE_URL"] = "http://localhost:3000",
                    ["GOTRUE_DB_DRIVER"] = "postgres", ["GOTRUE_DB_DATABASE_URL"] = $"postgresql://supabase_auth_admin:{secrets.DatabasePassword}@127.0.0.1:54322/postgres",
                    ["GOTRUE_JWT_SECRET"] = secrets.JwtSecret, ["GOTRUE_JWT_AUD"] = "authenticated",
                    ["GOTRUE_JWT_DEFAULT_GROUP_NAME"] = "authenticated", ["GOTRUE_JWT_ADMIN_ROLES"] = "service_role",
                    ["GOTRUE_EXTERNAL_EMAIL_ENABLED"] = "true", ["GOTRUE_MAILER_AUTOCONFIRM"] = "true",
                }, "auth");
                await WaitHttpAsync("http://127.0.0.1:54328/health", stoppingToken);
                available.Add("Auth");
                Volatile.Write(ref status, Status with { Unavailable = Status.Unavailable.Where(s => s != "Auth").ToArray() });
            }
            Update("starting", "Starting upstream Supabase Studio");
            var studioEnv = new Dictionary<string, string>
            {
                ["HOSTNAME"] = "127.0.0.1", ["PORT"] = "54323", ["NODE_ENV"] = "production",
                ["NEXT_PUBLIC_IS_PLATFORM"] = "false", ["STUDIO_PG_META_URL"] = "http://127.0.0.1:54324",
                ["POSTGRES_HOST"] = "127.0.0.1", ["POSTGRES_PORT"] = "54322", ["POSTGRES_DB"] = "postgres",
                ["POSTGRES_PASSWORD"] = secrets.DatabasePassword, ["POSTGRES_USER_READ_WRITE"] = "postgres",
                ["PG_META_CRYPTO_KEY"] = secrets.MetaCryptoKey, ["AUTH_JWT_SECRET"] = secrets.JwtSecret,
                ["SUPABASE_URL"] = "http://127.0.0.1:54321", ["SUPABASE_PUBLIC_URL"] = "http://127.0.0.1:54321",
                ["SUPABASE_ANON_KEY"] = keys.Anon!, ["SUPABASE_SERVICE_KEY"] = keys.Service!,
                ["DEFAULT_ORGANIZATION_NAME"] = "Native Windows", ["DEFAULT_PROJECT_NAME"] = "Local project",
                ["PGRST_DB_SCHEMAS"] = "public", ["ENABLED_FEATURES_LOGS_ALL"] = "false",
                ["SNIPPETS_MANAGEMENT_FOLDER"] = Path.Combine(data, "snippets"),
                ["EDGE_FUNCTIONS_MANAGEMENT_FOLDER"] = Path.Combine(data, "functions"),
            };
            Directory.CreateDirectory(studioEnv["SNIPPETS_MANAGEMENT_FOLDER"]);
            Directory.CreateDirectory(studioEnv["EDGE_FUNCTIONS_MANAGEMENT_FOLDER"]);
            Start("node/node.exe", [Path.Combine(root, "studio/server.js")], Path.Combine(root, "studio"), studioEnv, "studio");
            await WaitHttpAsync("http://127.0.0.1:54323/api/platform/profile", stoppingToken);
            Update("partial", "Studio and database are ready. Other Supabase services are still being ported.", "http://127.0.0.1:54323", available.ToArray());
            while (!stoppingToken.IsCancellationRequested)
            {
                await Task.Delay(TimeSpan.FromSeconds(3), stoppingToken);
                if (children.Any(p => p.HasExited)) throw new InvalidOperationException("A native service exited. Check logs in the data directory.");
            }
        }
        catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested) { }
        catch (Exception ex)
        {
            logger.LogError(ex, "Native stack startup failed");
            Update("failed", ex.Message);
        }
        finally
        {
            foreach (var child in children.AsEnumerable().Reverse())
            {
                try { if (!child.HasExited) child.Kill(entireProcessTree: true); }
                catch (InvalidOperationException) { }
                child.Dispose();
            }
            var pgData = Path.Combine(data, "postgres");
            if (postgresOwned && File.Exists(Path.Combine(pgData, "postmaster.pid")))
            {
                try { await RunAsync("postgres/bin/pg_ctl.exe", ["-D", pgData, "-m", "fast", "-w", "stop"], null, CancellationToken.None); }
                catch (Exception ex) { logger.LogWarning(ex, "Could not stop private PostgreSQL"); }
            }
            instanceLock?.Dispose();
        }
    }

    private ProcessStartInfo Info(string file, string[] args, string? cwd, Dictionary<string, string>? environment)
    {
        var info = new ProcessStartInfo(Path.Combine(root, file)) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true, WorkingDirectory = cwd ?? root };
        // PostgREST's Windows executable dynamically loads libpq from PostgreSQL.
        info.Environment["PATH"] = Path.Combine(root, "postgres/bin") + Path.PathSeparator + info.Environment["PATH"];
        foreach (var arg in args) info.ArgumentList.Add(arg);
        if (environment is not null) foreach (var pair in environment) info.Environment[pair.Key] = pair.Value;
        return info;
    }
    private async Task RunAsync(string file, string[] args, Dictionary<string, string>? environment, CancellationToken token)
    {
        var info = Info(file, args, null, environment);
        var detached = file.EndsWith("pg_ctl.exe", StringComparison.OrdinalIgnoreCase);
        // On Windows pg_ctl's detached server inherits pipe handles. Waiting for
        // EOF on redirected pipes would wait for PostgreSQL itself to stop.
        if (detached) { info.RedirectStandardOutput = false; info.RedirectStandardError = false; }
        using var process = Process.Start(info) ?? throw new InvalidOperationException("Could not launch " + file);
        var stdout = detached ? Task.FromResult("") : process.StandardOutput.ReadToEndAsync(token);
        var stderr = detached ? Task.FromResult("") : process.StandardError.ReadToEndAsync(token);
        try { await process.WaitForExitAsync(token); }
        catch { if (!process.HasExited) process.Kill(true); throw; }
        await stdout;
        var error = await stderr;
        if (process.ExitCode != 0) throw new InvalidOperationException($"{Path.GetFileName(file)} failed ({process.ExitCode}): {error}");
    }
    private void Start(string file, string[] args, string cwd, Dictionary<string, string> environment, string name)
    {
        var process = Process.Start(Info(file, args, cwd, environment)) ?? throw new InvalidOperationException("Could not launch " + name);
        children.Add(process);
        _ = CaptureAsync(process.StandardOutput, name);
        _ = CaptureAsync(process.StandardError, name);
    }
    private async Task CaptureAsync(StreamReader reader, string name)
    {
        try
        {
            using var writer = new StreamWriter(Path.Combine(data, name + "-" + Guid.NewGuid().ToString("N") + ".log"));
            writer.AutoFlush = true;
            while (await reader.ReadLineAsync() is { } line) await writer.WriteLineAsync(line);
        }
        catch (ObjectDisposedException) { }
        catch (IOException ex) { logger.LogWarning(ex, "Service log capture failed"); }
    }
    private async Task WaitHttpAsync(string url, CancellationToken token)
    {
        for (var attempt = 0; attempt < 90; attempt++)
        {
            token.ThrowIfCancellationRequested();
            if (children.Any(p => p.HasExited)) throw new InvalidOperationException("Service exited before becoming healthy.");
            try { using var response = await http.GetAsync(url, token); if (response.IsSuccessStatusCode) return; }
            catch (HttpRequestException) { }
            catch (TaskCanceledException) when (!token.IsCancellationRequested) { }
            await Task.Delay(1000, token);
        }
        throw new TimeoutException("Service did not become healthy: " + url);
    }
    public override void Dispose() { http.Dispose(); base.Dispose(); }
}

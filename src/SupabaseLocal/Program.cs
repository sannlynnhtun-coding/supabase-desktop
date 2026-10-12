using SupabaseLocal;
using Yarp.ReverseProxy.Configuration;

var builder = WebApplication.CreateBuilder(args);
builder.WebHost.UseUrls("http://127.0.0.1:54320", "http://127.0.0.1:54321");
builder.Services.AddSingleton<ApiKeys>();
builder.Services.Configure<HostOptions>(options => options.ShutdownTimeout = TimeSpan.FromSeconds(90));
builder.Services.AddReverseProxy().LoadFromMemory(
    new[]
    {
        new RouteConfig { RouteId = "rest", ClusterId = "rest", Match = new() { Path = "/rest/v1/{**remainder}" }, Transforms = [new Dictionary<string, string> { ["PathRemovePrefix"] = "/rest/v1" }] },
        new RouteConfig { RouteId = "auth", ClusterId = "auth", Match = new() { Path = "/auth/v1/{**remainder}" }, Transforms = [new Dictionary<string, string> { ["PathRemovePrefix"] = "/auth/v1" }] },
    },
    new[]
    {
        new ClusterConfig { ClusterId = "rest", Destinations = new Dictionary<string, DestinationConfig> { ["local"] = new() { Address = "http://127.0.0.1:54326/" } } },
        new ClusterConfig { ClusterId = "auth", Destinations = new Dictionary<string, DestinationConfig> { ["local"] = new() { Address = "http://127.0.0.1:54328/" } } },
    });
builder.Services.AddCors(options => options.AddDefaultPolicy(policy => policy.SetIsOriginAllowed(origin => Uri.TryCreate(origin, UriKind.Absolute, out var uri) && (uri.Host == "localhost" || uri.Host == "127.0.0.1")).AllowAnyHeader().AllowAnyMethod()));
builder.Services.AddSingleton<NativeStack>();
builder.Services.AddHostedService(sp => sp.GetRequiredService<NativeStack>());
var app = builder.Build();
// Reject DNS rebinding even though Kestrel only listens on loopback.
app.Use(async (context, next) =>
{
    if (context.Request.Host.Host != "127.0.0.1" && context.Request.Host.Host != "localhost")
    {
        context.Response.StatusCode = 403;
        return;
    }
    await next(context);
});
app.UseDefaultFiles();
app.UseStaticFiles();
app.UseCors();
app.Use(async (context, next) =>
{
    if (context.Request.Path.StartsWithSegments("/rest/v1") || context.Request.Path.StartsWithSegments("/auth/v1"))
    {
        var keys = context.RequestServices.GetRequiredService<ApiKeys>();
        var key = context.Request.Headers["apikey"].ToString();
        if (!keys.IsValid(key)) { context.Response.StatusCode = 401; return; }
        if (!context.Request.Headers.ContainsKey("Authorization")) context.Request.Headers.Authorization = "Bearer " + key;
    }
    await next(context);
});
app.MapReverseProxy();
app.MapGet("/api/status", (NativeStack stack) => stack.Status);
app.MapGet("/health", () => Results.Ok(new { manager = "running" }));
app.MapPost("/api/shutdown", (HttpContext context, IConfiguration config, IHostApplicationLifetime lifetime) =>
{
    var expected = config["LauncherToken"];
    if (string.IsNullOrEmpty(expected) || context.Request.Headers["X-Launcher-Token"] != expected) return Results.Unauthorized();
    lifetime.StopApplication();
    return Results.Ok();
});
app.Run();

using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace SupabaseLocal;

public sealed class ApiKeys
{
    public string? Anon { get; private set; }
    public string? Service { get; private set; }
    public void Initialize(string secret)
    {
        Anon = Sign("anon", secret);
        Service = Sign("service_role", secret);
    }
    public bool IsValid(string key) => !string.IsNullOrEmpty(key) && (key == Anon || key == Service);
    private static string Sign(string role, string secret)
    {
        static string Encode(byte[] bytes) => Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');
        var header = Encode(Encoding.UTF8.GetBytes("{\"alg\":\"HS256\",\"typ\":\"JWT\"}"));
        var payload = Encode(JsonSerializer.SerializeToUtf8Bytes(new { role, iss = "supabase", iat = 0L, exp = 4102444800L }));
        var input = header + "." + payload;
        return input + "." + Encode(HMACSHA256.HashData(Encoding.UTF8.GetBytes(secret), Encoding.ASCII.GetBytes(input)));
    }
}

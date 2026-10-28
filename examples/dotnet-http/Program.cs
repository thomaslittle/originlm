using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;

if (args.Length != 1 || !Uri.TryCreate(args[0], UriKind.Absolute, out var endpoint)
    || endpoint.Scheme != "http" || !IPAddress.TryParse(endpoint.Host, out var ip)
    || !IPAddress.IsLoopback(ip) || endpoint.UserInfo.Length != 0
    || endpoint.Query.Length != 0 || endpoint.Fragment.Length != 0)
{
    Console.Error.WriteLine("Usage: dotnet run -- http://127.0.0.1:PORT");
    return 2;
}
using var client = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false, UseProxy = false })
{ BaseAddress = endpoint, Timeout = TimeSpan.FromSeconds(30), MaxResponseContentBufferSize = 1048576 };
Console.Write("One-use pairing code from your local helper: ");
var code = Console.ReadLine();
if (code is null || code.Length != 12) return 2;
async Task<JsonDocument> Post(string path, object value)
{
    using var body = new StringContent(JsonSerializer.Serialize(value), Encoding.UTF8, "application/json");
    using var response = await client.PostAsync(path, body);
    response.EnsureSuccessStatusCode();
    return JsonDocument.Parse(await response.Content.ReadAsStringAsync());
}
using var pairing = await Post("/v1/pair", new { code });
var token = pairing.RootElement.GetProperty("token").GetString();
client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);
var sequence = 0;
async Task<JsonDocument> Rpc(string method, object parameters)
{
    var value = await Post("/v1/rpc", new { jsonrpc = "2.0", id = ++sequence, method, @params = parameters });
    if (value.RootElement.TryGetProperty("error", out _)) { value.Dispose(); throw new InvalidOperationException("Native request was denied or unavailable."); }
    return value;
}
try
{
    using var handshake = await Rpc("runtime.handshake", new { protocolVersion = "1.0", clientName = "dotnet-example" });
    Console.Write("Run one passive inventory scan (no account probe or inference)? [y/N] ");
    if (Console.ReadLine()?.Equals("y", StringComparison.OrdinalIgnoreCase) == true)
    { using var scan = await Rpc("discovery.refresh", new { level = "passive" }); }
    using var snapshot = await Rpc("snapshot.get", new { });
    Console.WriteLine(snapshot.RootElement.GetProperty("result").GetRawText());
}
finally
{
    try { using var close = await Rpc("runtime.closeClient", new { }); }
    catch (HttpRequestException) { }
}
return 0;

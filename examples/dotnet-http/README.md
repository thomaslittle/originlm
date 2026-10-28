# C# HTTP consumer

A .NET 8 console example of the documented local protocol, not a separate C# SDK. Start an authorized loopback helper, run `dotnet run -- http://127.0.0.1:PORT`, and enter its one-use pairing code. The program optionally runs passive discovery and reads a snapshot; it never sends inference, metadata-probe, arbitrary-file, or execution requests.

The source requires a .NET SDK. The development Windows host had only .NET runtimes, so this example is source-reviewed but **not locally compiled/qualified**. Do not confuse the separately passing native HTTP protocol tests with C# compiler verification.

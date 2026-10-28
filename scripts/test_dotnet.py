"""Compile and run the plain HTTP example against an isolated native host; no accounts."""
import json, os, pathlib, re, shutil, subprocess, tempfile
ROOT = pathlib.Path(__file__).resolve().parents[1]
ENV = {**os.environ, 'DOTNET_CLI_TELEMETRY_OPTOUT':'1', 'DOTNET_GENERATE_ASPNET_CERTIFICATE':'false', 'DOTNET_NOLOGO':'1'}
suffix = '.exe' if os.name == 'nt' else ''
local = ROOT / '.verification/dotnet' / ('dotnet'+suffix)
dotnet = os.environ.get('LAI_TEST_DOTNET') or (str(local) if local.is_file() else shutil.which('dotnet'))
if not dotnet:
    raise SystemExit('A .NET 8 SDK is required; set LAI_TEST_DOTNET to its executable.')
subprocess.run([dotnet, 'build', str(ROOT/'examples/dotnet-http/LocalAI.Example.csproj'), '-c', 'Release', '--nologo'], cwd=ROOT, env=ENV, check=True, timeout=120)
dll = ROOT/'examples/dotnet-http/bin/Release/net8.0/LocalAI.Example.dll'
binary = pathlib.Path(os.environ.get('LAI_NATIVE_BINARY', ROOT/'target/debug'/('originlm'+suffix)))
with tempfile.TemporaryDirectory(prefix='originlm-dotnet-home-') as temp:
    host = subprocess.Popen([str(binary), 'serve', '--http', '--port', '0', '--home', temp, '--allow-passive-discovery'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    try:
        url_line = host.stdout.readline()
        code_line = host.stdout.readline()
        url = re.search(r'http://127\.0\.0\.1:\d+', url_line)
        code = re.search(r'([0-9a-f]{12})\s*$', code_line)
        assert url and code, 'Native test host did not publish a pairing endpoint'
        result = subprocess.run([dotnet, str(dll), url[0]], input=code[1]+'\ny\n', capture_output=True, text=True, env=ENV, check=True, timeout=30)
        snapshot = json.loads(result.stdout[result.stdout.index('{'):])
        assert snapshot['mode']=='live' and snapshot['connections']==[] and snapshot['models']==[]
        denied = subprocess.run([dotnet,str(dll),'https://example.invalid/'],capture_output=True,text=True,env=ENV,timeout=10)
        assert denied.returncode==2, 'The example must reject remote endpoints before pairing'
        print('PASS C# -> real native HTTP pairing, passive scan, and owner cleanup; remote endpoint rejected')
    finally:
        host.terminate()
        host.wait(timeout=5)

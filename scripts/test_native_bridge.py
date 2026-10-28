"""Build the actual native helper and exercise it from both SDKs. No accounts required."""
import os, pathlib, shutil, subprocess, sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run(command,env=None):
    executable=shutil.which(command[0])
    if not executable: raise RuntimeError('Missing executable: '+command[0])
    if os.name=='nt' and executable.lower().endswith(('.cmd','.bat')):
        command=[os.environ.get('COMSPEC','cmd.exe'),'/d','/c',executable,*command[1:]]
    subprocess.run(command,cwd=ROOT,env=env,check=True,timeout=180)
def main():
    run(['cargo','build','--locked','-p','originlm-cli'])
    run(['npm','run','build'])
    target=pathlib.Path(os.environ.get('CARGO_TARGET_DIR',str(ROOT/'target')))
    if not target.is_absolute():target=ROOT/target
    binary=target/'debug'/('originlm.exe' if os.name=='nt' else 'originlm')
    if not binary.is_file():raise RuntimeError('Compiled native helper is missing')
    env={**os.environ,'LAI_NATIVE_BINARY':str(binary.resolve())}
    run(['node','--test','tests/integration/sdk-native.test.mjs','tests/integration/broker-native.test.mjs'],env)
    run([sys.executable,'tests/integration/test_sdk_native.py','-v'],env)
    run([sys.executable,'tests/integration/test_broker_native.py','-v'],env)
if __name__=='__main__':main()

"""Run actual repository checks; absent tools or failed suites fail this profile."""
import argparse,json,os,pathlib,shutil,subprocess,sys,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
RECORDS=[]
def run(command,required=None):
    if required and not (ROOT/required).exists():raise RuntimeError('Missing required implementation: '+required)
    local=ROOT/'node_modules/.bin'/(str(command[0])+('.cmd' if os.name=='nt' else ''))
    exe=str(local) if local.is_file() else shutil.which(str(command[0]))
    if not exe:raise RuntimeError('Missing tool: '+str(command[0]))
    args=[exe,*map(str,command[1:])]
    if os.name=='nt' and exe.lower().endswith(('.cmd','.bat')):args=[os.environ.get('COMSPEC','cmd.exe'),'/d','/c',*args]
    env={**os.environ,'CARGO_INCREMENTAL':'0','LAI_TEST_PYTHON':sys.executable,'PYTHONUTF8':'1'}
    # cargo uses Python only as a test peer; it never discovers or calls an AI account.
    start=time.perf_counter();result=subprocess.run(args,cwd=ROOT,env=env)
    RECORDS.append({'command':list(map(str,command)),'exitCode':result.returncode,'seconds':round(time.perf_counter()-start,3)})
    if result.returncode:raise subprocess.CalledProcessError(result.returncode,command)

def js_package(name):
    run(['tsc','-p',f'packages/{name}/tsconfig.json'])
    tests=sorted((ROOT/'packages'/name/'tests').glob('*.test.mjs'))
    if not tests:raise RuntimeError('No tests for '+name)
    run(['node','--experimental-strip-types','--test',*tests])

def main():
    p=argparse.ArgumentParser();p.add_argument('--profile',choices=['core','adapters','selectors','sdk-typescript','sdk','javascript','react','generation','integration','ui','release','all'],default='core');p.add_argument('--adapter');a=p.parse_args();full=a.profile in ('all','release')
    if a.profile=='core' or full:run(['cargo','test','--workspace','--all-targets','--locked','--no-fail-fast'],'Cargo.toml')
    if a.profile=='adapters':run([sys.executable,'scripts/adapter_conformance.py',*(['--adapter',a.adapter] if a.adapter else ['--all'])])
    if a.profile in ('selectors','javascript') or full:js_package('selector-core')
    if a.profile in ('sdk-typescript','sdk','javascript') or full:
        js_package('sdk-typescript')
        js_package('sdk-web')
    if a.profile in ('sdk',) or full:run([sys.executable,'-m','unittest','discover','-s','packages/sdk-python/tests','-v'])
    if a.profile in ('generation',) or full:
        run([sys.executable,'-m','unittest','discover','-s','tests/generation','-v'])
        run([sys.executable,'scripts/check_generated.py'])
        run(['tsc','-p','tests/generation/tsconfig.json'])
    if a.profile=='react' or full:
        run(['npm','run','build']);run(['npm','run','test:react'])
    if a.profile=='integration' or full:
        run([sys.executable,'scripts/test_native_bridge.py'])
        run(['cargo','run','--locked','-p','originlm-kit','--example','synthetic_adapter'])
        run([sys.executable,'-m','unittest','discover','-s','tests/packaging','-v'])
        run([sys.executable,'scripts/check_project.py'])
        run(['node','--experimental-strip-types','--test','tests/integration/cached-usage-mcp.test.mjs','tests/integration/cached-usage-host.test.mjs'])
        run(['node','--experimental-strip-types','examples/mcp/remote/inert-check.mjs'])
    if a.profile=='ui' or full:
        run(['npm','run','build']);run(['npm','run','doctor']);run(['node','scripts/build_demo.mjs']);run(['node','scripts/build_extension.mjs']);run(['cargo','build','--locked','-p','originlm-cli'])
        run(['node','--test',*sorted((ROOT/'apps/demo/tests').glob('*.test.mjs'))])
        run([sys.executable,'scripts/test_demo.py'])
        run(['node','--test','scripts/test_live_demo.mjs','scripts/test_live_demo_opencode.mjs','scripts/test_demo_profile_auth.mjs','scripts/test_demo_errors.mjs','scripts/test_demo_catalog_visibility.mjs'])
        run(['node','--test','scripts/test_accessibility.mjs','scripts/test_component_accessibility.mjs'])
    if a.profile=='release':
        run([sys.executable,'scripts/test_dotnet.py'])
        run(['cargo','fmt','--all','--check']);run(['cargo','clippy','--workspace','--all-targets','--all-features','--','-D','warnings'])
        run([sys.executable,'scripts/adapter_conformance.py','--all'])
        run([sys.executable,'scripts/build_release.py','--profile','release'])
        run([sys.executable,'scripts/package_smoke.py'])
        run([sys.executable,'scripts/benchmark.py','--idle-seconds','60'])
    print('Profile completed: '+a.profile)
    return 0
if __name__=='__main__':
    result=1
    try:result=main()
    except (RuntimeError,subprocess.CalledProcessError,OSError)as e:print(str(e),file=sys.stderr);result=getattr(e,'returncode',1)
    finally:
        directory=ROOT/'.verification';directory.mkdir(exist_ok=True)
        (directory/'verify-last.json').write_text(json.dumps({'exitCode':result,'commands':RECORDS},indent=2)+'\n',encoding='utf-8')
    raise SystemExit(result)

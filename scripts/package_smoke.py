"""Verify distributable artifacts from clean consumers, without inference or publication."""
import argparse,json,os,pathlib,shutil,subprocess,sys,tempfile,zipfile
from release_support import verify_manifest
ROOT=pathlib.Path(__file__).resolve().parents[1]

def run(command,cwd,env=None):
    exe=shutil.which(str(command[0]))
    if not exe:raise RuntimeError('Missing consumer tool: '+str(command[0]))
    args=[exe,*map(str,command[1:])]
    if os.name=='nt' and exe.lower().endswith(('.cmd','.bat')):args=[os.environ.get('COMSPEC','cmd.exe'),'/d','/c',*args]
    result=subprocess.run(args,cwd=cwd,env=env,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=240)
    if result.returncode:raise RuntimeError('Consumer check failed: '+str(command[0])+'\n'+result.stdout[-6000:]+result.stderr[-6000:])
    return result.stdout

def peer_packages(root):
    """Use installed peer packages without weakening dependency resolution."""
    import re
    tree=(root/'node_modules').resolve();pending=['react','react-dom'];found={}
    while pending:
        name=pending.pop()
        if not re.fullmatch(r'(?:@[a-z0-9_.-]+/)?[a-z0-9][a-z0-9_.-]*',name):raise ValueError('Invalid peer package name')
        if name in found:continue
        if len(found)>=32:raise ValueError('Peer dependency graph exceeds its test budget')
        path=(tree/name).resolve()
        if not path.is_relative_to(tree) or not (path/'package.json').is_file():raise ValueError('Required installed peer is missing')
        metadata=json.loads((path/'package.json').read_text(encoding='utf-8'))
        if metadata.get('name')!=name:raise ValueError('Installed peer identity mismatch')
        found[name]=path
        pending.extend(metadata.get('dependencies',{}))
    return [found[name] for name in sorted(found)]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--bundle',type=pathlib.Path);args=parser.parse_args()
    if args.bundle:bundle=args.bundle.resolve()
    else:
        pointer=ROOT/'dist/release/latest.json'
        if not pointer.is_file():raise RuntimeError('Build a release bundle first: python scripts/build_release.py')
        bundle=(ROOT/json.loads(pointer.read_text(encoding='utf-8'))['directory']).resolve()
    manifest=json.loads((bundle/'manifest.json').read_text(encoding='utf-8'));verify_manifest(bundle,manifest)
    suffix='.exe' if os.name=='nt' else '';binary=bundle/'bin'/('originlm'+suffix)
    with tempfile.TemporaryDirectory(prefix='originlm-clean-consumer-') as directory:
        base=pathlib.Path(directory);home=base/'empty-home';home.mkdir()
        # This executable is copied out of the workspace's target directory.
        assert 'originlm' in run([binary,'--version'],base)
        snapshot=json.loads(run([binary,'scan','--json','--home',home],base));assert snapshot['mode']=='live' and not snapshot['connections'] and not snapshot['models']
        consumer=base/'node';consumer.mkdir()
        package_paths=sorted((bundle/'packages').glob('*.tgz'))
        if len(package_paths)!=4:raise RuntimeError('Expected four JavaScript artifacts')
        (consumer/'package.json').write_text(json.dumps({'private':True,'type':'module'}),encoding='utf-8')
        peers_dir=base/'peers';peers_dir.mkdir()
        for peer in peer_packages(ROOT):
            run(['npm','pack',peer,'--ignore-scripts','--json','--pack-destination',peers_dir],ROOT)
        peer_paths=sorted(peers_dir.glob('*.tgz'))
        run(['npm','install','--offline','--ignore-scripts','--no-audit','--no-fund',*package_paths,*peer_paths],consumer)
        script="import assert from 'node:assert/strict';\nimport {createLocalAI,createSnapshotStore} from 'originlm';\nimport {openBundledTransport} from 'originlm/node';\nimport {createCatalog} from '@originlm/selector-core';\nimport {LocalAIProvider} from '@originlm/react';\nimport {ProviderModelSelector} from '@originlm/ui-react';\nimport React from 'react';\nimport {renderToString} from 'react-dom/server';\nconst t=await openBundledTransport({bundleDirectory:process.argv[2],args:['serve','--stdio','--home',process.argv[3],'--allow-passive-discovery']});\nconst ai=await createLocalAI({transport:t});\ntry {\n assert.equal((await ai.snapshot.get()).connections.length,0);\n await ai.discovery.refresh({level:'passive'});\n const snapshot=await ai.snapshot.get();assert.equal(snapshot.mode,'live');assert.equal(snapshot.connections.length,0);\n assert.equal(createCatalog(snapshot).models.size,0);\n const store=createSnapshotStore(ai);assert.equal(store.getSnapshot().snapshot,null);assert.equal(store.getSnapshot().loading,false);\n const html=renderToString(React.createElement(LocalAIProvider,{store},React.createElement(ProviderModelSelector,{snapshot,value:null,onValueChange:()=>{},variant:'rail'})));\n assert.ok(html.length>0);store.dispose();\n console.log('PASS packaged JS SDK/native scan and React server rendering');\n} finally {await ai.close();}\n"
        (consumer/'consumer.mjs').write_text(script,encoding='utf-8')
        js=run(['node',consumer/'consumer.mjs',bundle,home],consumer)
        wheel=list((bundle/'packages').glob('*.whl'))
        if len(wheel)!=1:raise RuntimeError('Expected one Python wheel')
        with zipfile.ZipFile(wheel[0]) as archive:
            names=archive.namelist()
            for required in ('originlm/client.pyi','originlm/generated.py','originlm/py.typed'):
                if required not in names:raise RuntimeError('Wheel lacks '+required)
        destination=base/'python';destination.mkdir()
        run([sys.executable,'-m','pip','install','--no-index','--no-deps','--target',destination,wheel[0]],base)
        program="import pathlib,sys\nsys.path.insert(0,sys.argv[1])\nimport originlm\nfrom originlm import Client, resolve_bundle_binary\nfrom originlm.generated import Snapshot\nassert pathlib.Path(originlm.__file__).resolve().is_relative_to(pathlib.Path(sys.argv[1]).resolve())\nwith Client.from_stdio(executable=resolve_bundle_binary(sys.argv[2]),args=['serve','--stdio','--home',sys.argv[3],'--allow-passive-discovery']) as ai:\n    assert ai.snapshot.get()['connections']==[]\n    ai.discovery.refresh({'level':'passive'})\n    assert ai.snapshot.get()['mode']=='live'\n    assert ai.models.list()==[]\nprint('PASS installed Python wheel with actual native helper')\n"
        script_path=base/'consumer.py';script_path.write_text(program,encoding='utf-8')
        py=run([sys.executable,'-I',script_path,destination,bundle,home],base)
    report={'sourceCommit':manifest['sourceCommit'],'workingTreeDirty':manifest['workingTreeDirty'],'platform':manifest['platform'],'checks':{'manifest':True,'standaloneNative':True,'npmTarballs':True,'pythonWheel':True,'reactSSR':True},'inferenceRequests':0,'limitations':['No remote provider inference or OS signing verification','Registry publication was not attempted']}
    out=ROOT/'.verification/package-smoke.json';out.parent.mkdir(exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(js.strip());print(py.strip());print(json.dumps(report,indent=2))
if __name__=='__main__':
    try:main()
    except (AssertionError,ValueError,RuntimeError,OSError,subprocess.SubprocessError)as error:print(str(error),file=sys.stderr);raise SystemExit(1)

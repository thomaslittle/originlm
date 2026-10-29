"""Build local distributable bundles. Does not publish, sign, or use provider accounts."""
import argparse,json,os,pathlib,platform,re,shutil,subprocess,sys,tempfile,tomllib
from license_closure import collect_many,write_assets
from release_support import make_manifest,verify_manifest,create_archive,sha256
ROOT=pathlib.Path(__file__).resolve().parents[1]
PACKAGES=('sdk-typescript','selector-core','react','ui-react')

def run(command,*,cwd=ROOT):
    exe=shutil.which(command[0])
    if not exe:raise RuntimeError('Missing build tool: '+command[0])
    args=[exe,*map(str,command[1:])]
    if os.name=='nt' and exe.lower().endswith(('.cmd','.bat')):args=[os.environ.get('COMSPEC','cmd.exe'),'/d','/c',*args]
    result=subprocess.run(args,cwd=cwd,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=1200)
    if result.returncode:raise RuntimeError('Build command failed: '+command[0]+'\n'+result.stdout[-6000:]+result.stderr[-6000:])
    return result.stdout

def write(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(data,indent=2,sort_keys=True)+'\n',encoding='utf-8')

def dependency_inventory(stage,target,head):
    # Only ship and inventory the Rust binaries and JavaScript packages that are
    # actually in this bundle. Cargo metadata supplies exact package/license
    # identity; collect_many resolves each shipped binary's feature-accurate
    # normal/build closure and fails if a required license text is missing.
    collection=collect_many(ROOT,target,['originlm-cli','originlm-control'])
    license_source=ROOT/'LICENSE';data=license_source.read_bytes();digest=sha256(license_source)
    for package in PACKAGES:
        info=json.loads((ROOT/'packages'/package/'package.json').read_text(encoding='utf-8'))
        name=info['name'];version=info['version'];expression=info.get('license')
        if not expression:raise RuntimeError('JavaScript package has no license expression: '+name)
        directory=re.sub(r'[^A-Za-z0-9._+-]','_',name+'-'+version)
        member=f'licenses/{directory}/LICENSE'
        collection['components'].append({
            'name':name,'version':version,'purl':f'pkg:npm/{name}@{version}',
            'license':expression,'source':{'kind':'workspace','path':f'packages/{package}'},
            'licenseTexts':[{'path':member,'bytes':len(data),'sha256':digest,'sourcePath':'LICENSE','_source':str(license_source)}],
        })
        text=next((item for item in collection['texts'] if item['digest']==digest),None)
        if text is None:
            collection['texts'].append({'digest':digest,'text':data.decode('utf-8'),'paths':[member]})
        elif member not in text['paths']:
            text['paths'].append(member)
    collection['components'].sort(key=lambda item:item['purl'])
    write_assets(stage,collection,head)

def build(profile):
    run(['cargo','build','--locked','-p','originlm-cli','-p','originlm-control',*(['--release'] if profile=='release' else [])])
    run(['npm','run','build']);run(['node','scripts/build_demo.mjs']);run(['node','scripts/build_extension.mjs'])
    head=run(['git','rev-parse','HEAD']).strip();dirty=bool(run(['git','status','--porcelain']).strip())
    version=json.loads((ROOT/'package.json').read_text(encoding='utf-8'))['version']
    target=next(line.split(': ',1)[1] for line in run(['rustc','-vV']).splitlines() if line.startswith('host: '))
    normalized_os=platform.system()
    normalized_os={'Windows':'windows','Darwin':'macos','Linux':'linux'}.get(normalized_os, normalized_os.lower())
    normalized_arch={'AMD64':'x86_64','x86_64':'x86_64','arm64':'aarch64','aarch64':'aarch64'}.get(platform.machine(), platform.machine().lower())
    update_platform=f'{normalized_os}-{normalized_arch}'
    out=ROOT/'dist/release';out.mkdir(parents=True,exist_ok=True)
    name=f'originlm-{version}-{target}-{head[:12]}'+('-working' if dirty else '')
    final=out/name
    with tempfile.TemporaryDirectory(prefix='.build-',dir=out) as temp:
        stage=pathlib.Path(temp)/name;stage.mkdir()
        target_dir=pathlib.Path(os.environ.get('CARGO_TARGET_DIR',ROOT/'target'))
        if not target_dir.is_absolute():target_dir=ROOT/target_dir
        binaries=target_dir/('release' if profile=='release' else 'debug');(stage/'bin').mkdir()
        for binary in ('originlm','originlm-native-host','originlm-auth-blocker','originlm-control'):
            filename=binary+('.exe' if os.name=='nt' else '');shutil.copy2(binaries/filename,stage/'bin'/filename)
        shutil.copytree(ROOT/'apps/demo/dist',stage/'demo');shutil.copytree(ROOT/'dist/browser-extension',stage/'browser-extension')
        for path in ('LICENSE','THIRD_PARTY_NOTICES.md','SECURITY.md'):shutil.copy2(ROOT/path,stage/path)
        shutil.copytree(ROOT/'protocol',stage/'protocol')
        (stage/'packages').mkdir()
        for package in PACKAGES:
            info=json.loads(run(['npm','pack','--ignore-scripts','--json','--pack-destination',stage/'packages'],cwd=ROOT/'packages'/package));
            if len(info)!=1:raise RuntimeError('Unexpected package output')
        run([sys.executable,'-m','pip','wheel','--no-deps','--no-build-isolation','--wheel-dir',stage/'packages',ROOT/'packages/sdk-python'])
        shutil.copytree(ROOT/'examples',stage/'examples',ignore=shutil.ignore_patterns('__pycache__','node_modules','target','bin','obj'))
        (stage/'scripts').mkdir();shutil.copy2(ROOT/'scripts/native_host.py',stage/'scripts/native_host.py')
        shutil.copy2(ROOT/'scripts/release_support.py',stage/'scripts/release_support.py')
        dependency_inventory(stage,target,head)
        readme="# OriginLM local build\n\nThis archive is an unpublished developer build. Check `manifest.json` for the exact source commit, dirty state, platform, and profile. Checksums detect corruption; the bundle is not digitally signed.\n\nRun `bin/originlm --help`, or `bin/originlm scan --json` for explicit metadata-only discovery. Serve the demo using `bin/originlm serve --http --assets ABSOLUTE_PATH_TO_THIS_BUNDLE/demo --allow-passive-discovery`. Enter the displayed one-use pairing code in the browser. No provider or inference is contacted at startup.\n\nTo permit a Codex account/catalog probe, add `--allow-metadata codex`. Actual turns additionally require `--allow-execution codex --workspace ABSOLUTE_WORKSPACE`, and explicit invocation in the UI. Other integrations have their own provider grants. Do not enable execution just to view account metadata.\n\nThe `packages` folder contains four npm tarballs and the Python wheel. Install all npm tarballs together into your application's project; React is an optional peer dependency. The Python wheel requires Python 3.11 or newer. The native helper needs no Node/Python runtime; the JS/Python examples do.\n\nThe browser extension is uninstalled and has no access until you explicitly prepare and register the native messaging host. See the repository's browser integration documentation. No automatic OS integration is performed.\n\nArtifacts and private account data are never part of this bundle. The dependency inventory and notices cover the shipped Rust binaries and JavaScript packages; they are not a vulnerability clearance or a legal certification.\n"
        (stage/'README.md').write_text(readme,encoding='utf-8')
        manifest=make_manifest(stage,{'version':version,'sourceCommit':head,'workingTreeDirty':dirty,'platform':target,'profile':profile,'executedOn':platform.system(),'signed':False})
        write(stage/'manifest.json',manifest);verify_manifest(stage,manifest)
        if final.exists():
            # Only remove our fixed project distribution name; no user-supplied destination.
            if final.is_symlink() or final.parent!=out:raise RuntimeError('Unsafe existing release destination')
            shutil.rmtree(final)
        stage.rename(final)
    archive=out/(name+'.zip');create_archive(final,archive)
    (out/(name+'.sha256')).write_text(sha256(archive)+'  '+archive.name+'\n',encoding='utf-8')
    target_dir=pathlib.Path(os.environ.get('CARGO_TARGET_DIR',ROOT/'target'))
    if not target_dir.is_absolute():target_dir=ROOT/target_dir
    binary_dir=target_dir/('release' if profile=='release' else 'debug')
    update_artifact=out/(f'originlm-{version}-{update_platform}'+('.exe' if os.name=='nt' else ''))
    shutil.copy2(binary_dir/('originlm.exe' if os.name=='nt' else 'originlm'),update_artifact)
    update_manifest={
        'schemaVersion':1,
        'version':version,
        'channel':'alpha',
        'platform':update_platform,
        'protocolVersion':'1.0',
        'archive':update_artifact.name,
        'archiveBytes':update_artifact.stat().st_size,
        'archiveSha256':sha256(update_artifact),
        'publishedAt':__import__('datetime').datetime.now(__import__('datetime').timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
        'notesUrl':None,
        'signature':'',
        'signed':False,
        'status':'unsigned; activation requires an external Ed25519 signature',
    }
    write(out/'update-manifest.unsigned.json',update_manifest)
    write(out/'latest.json',{'directory':str(final.relative_to(ROOT)),'archive':str(archive.relative_to(ROOT)),'sha256':sha256(archive),'sourceCommit':head,'workingTreeDirty':dirty,'updateManifest':str((out/'update-manifest.unsigned.json').relative_to(ROOT)),'updateArchive':str(update_artifact.relative_to(ROOT)),'updateArchiveSha256':sha256(update_artifact)})
    print(json.dumps({'bundle':str(final),'archive':str(archive),'profile':profile,'dirty':dirty,'updateManifest':str(out/'update-manifest.unsigned.json'),'updateArchive':str(update_artifact)}))
    return final
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--profile',choices=['debug','release'],default='release');args=p.parse_args()
    try:build(args.profile)
    except (ValueError,RuntimeError,OSError,subprocess.SubprocessError) as error:print(str(error),file=sys.stderr);raise SystemExit(1)

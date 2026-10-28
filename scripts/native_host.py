"""Explicit native-host preparation/registration. No install side effect on import.
Generate into a NEW directory by default. --install registers the generated manifest
for the current user only; --unregister removes only an unchanged registration.
"""
import argparse,hashlib,json,os,pathlib,re,shutil,sys
NAME='com.originlm.host'
def prepare(binary,destination,extension_id,args=None,browser='chrome'):
    if browser=='firefox':
        if not re.fullmatch('[A-Za-z0-9._-]{1,64}@[A-Za-z0-9.-]{1,64}',extension_id):raise ValueError('Invalid Firefox extension ID')
    elif not re.fullmatch('[a-p]{32}',extension_id):raise ValueError('Invalid extension ID')
    binary=pathlib.Path(binary).resolve(strict=True);destination=pathlib.Path(destination).absolute()
    if not binary.is_file():raise ValueError('Native binary required')
    args=['--allow-passive-discovery'] if args is None else list(args)
    if len(args)>128 or any(not isinstance(v,str) or '\0' in v for v in args):raise ValueError('Invalid host arguments')
    destination.mkdir(mode=0o700,parents=True,exist_ok=False)
    target=destination/('originlm-native-host.exe' if os.name=='nt' else 'originlm-native-host')
    try:
        shutil.copy2(binary,target);target.chmod(0o700)
        if browser=='firefox':
            config_data={'allowedExtensions':[extension_id],'args':args}
            manifest_data={'name':NAME,'description':'OriginLM authorized native connector','path':str(target),'type':'stdio','allowed_extensions':[extension_id]}
        else:
            origin='chrome-extension://'+extension_id+'/'
            config_data={'allowedOrigins':[origin],'args':args}
            manifest_data={'name':NAME,'description':'OriginLM authorized native connector','path':str(target),'type':'stdio','allowed_origins':[origin]}
        config=target.with_suffix('.json');config.write_text(json.dumps(config_data,indent=2)+'\n',encoding='utf-8');config.chmod(0o600)
        manifest=destination/(NAME+'.json');manifest.write_text(json.dumps(manifest_data,indent=2)+'\n',encoding='utf-8');manifest.chmod(0o600)
        receipt={'formatVersion':1,'files':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [target,config,manifest]}}
        (destination/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
        return manifest
    except BaseException:
        shutil.rmtree(destination)
        raise

def verify(manifest):
    manifest=pathlib.Path(manifest).resolve(strict=True)
    if manifest.name!=NAME+'.json':raise ValueError('Unexpected manifest')
    receipt=json.loads((manifest.parent/'receipt.json').read_text(encoding='utf-8'))
    for name,digest in receipt['files'].items():
        if pathlib.Path(name).name!=name or hashlib.sha256((manifest.parent/name).read_bytes()).hexdigest()!=digest:raise ValueError('Installation files changed; reconcile before registration')
    return manifest

def platform_target(browser):
    if browser not in ['chrome','chromium','edge','firefox']:raise ValueError('Unsupported browser')
    if os.name=='nt':
        if browser=='firefox':return r'Software\Mozilla\NativeMessagingHosts'+'\\'+NAME
        return {'chrome':r'Software\Google\Chrome','chromium':r'Software\Chromium','edge':r'Software\Microsoft\Edge'}[browser]+'\\NativeMessagingHosts\\'+NAME
    if browser=='firefox':
        return (pathlib.Path.home()/'Library/Application Support/Mozilla/NativeMessagingHosts' if sys.platform=='darwin' else pathlib.Path.home()/'.mozilla'/'native-messaging-hosts')/(NAME+'.json')
    if sys.platform=='darwin':base=pathlib.Path.home()/'Library/Application Support'/{'chrome':'Google/Chrome','chromium':'Chromium','edge':'Microsoft Edge'}[browser]
    else:base=pathlib.Path(os.environ.get('XDG_CONFIG_HOME',str(pathlib.Path.home()/'.config')))/{'chrome':'google-chrome','chromium':'chromium','edge':'microsoft-edge'}[browser]
    return base/'NativeMessagingHosts'/(NAME+'.json')

def register(manifest,browser,remove=False):
    manifest=verify(manifest);target=platform_target(browser)
    if os.name=='nt':
        import winreg
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER,target,0,winreg.KEY_READ) as key:current=winreg.QueryValueEx(key,'')[0]
        except FileNotFoundError:current=None
        if remove:
            if current!=str(manifest):raise ValueError('Registration is missing or belongs to another installation')
            winreg.DeleteKey(winreg.HKEY_CURRENT_USER,target)
        else:
            if current is not None:raise FileExistsError('Existing browser registration left unchanged')
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER,target)as key:winreg.SetValueEx(key,'',0,winreg.REG_SZ,str(manifest))
    else:
        if remove:
            if target.read_bytes()!=manifest.read_bytes():raise ValueError('Browser registration changed; left untouched')
            target.unlink()
        else:
            target.parent.mkdir(parents=True,exist_ok=True)
            with target.open('xb')as output:output.write(manifest.read_bytes())
            target.chmod(0o600)
    return target

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=pathlib.Path);parser.add_argument('--destination',type=pathlib.Path);parser.add_argument('--extension-id');parser.add_argument('--browser',choices=['chrome','chromium','edge','firefox'],default='chrome')
    parser.add_argument('--install',action='store_true');parser.add_argument('--unregister',type=pathlib.Path);parser.add_argument('--allow-metadata',action='append',default=[]);parser.add_argument('--allow-execution',action='append',default=[]);parser.add_argument('--workspace',type=pathlib.Path)
    args=parser.parse_args()
    if args.unregister:
        if args.install or args.binary or args.destination:parser.error('Unregister is a separate operation')
        register(args.unregister,args.browser,True);print('Removed the matching browser registration; payload files retained.');return
    if not args.binary or not args.destination or not args.extension_id:parser.error('binary, destination, and extension-id are required')
    flags=['--allow-passive-discovery']
    for provider in args.allow_metadata:flags.extend(['--allow-metadata',provider])
    if args.allow_execution:
        if not args.workspace or not set(args.allow_execution)<=set(args.allow_metadata):parser.error('Execution needs workspace and matching metadata grants')
        for provider in args.allow_execution:flags.extend(['--allow-execution',provider])
        flags.extend(['--workspace',str(args.workspace.resolve(strict=True))])
    manifest=prepare(args.binary,args.destination,args.extension_id,flags,args.browser)
    if args.install:register(manifest,args.browser)
    print('Native host manifest:',manifest)
    print('Registered for current user.' if args.install else 'Prepared only; no browser or registry settings changed. Add --install to register.')
if __name__=='__main__':main()

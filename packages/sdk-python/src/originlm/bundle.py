"""Explicit native-bundle integrity checks, not publisher signature verification."""
from pathlib import Path
import hashlib,json,os,platform,re,stat,sys
from .errors import LocalAIError

def _invalid():return LocalAIError('BUNDLE_INVALID')

def resolve_bundle_binary(directory: str) -> str:
    try:
        if not isinstance(directory,str) or '\0' in directory:raise _invalid()
        root=Path(directory)
        if not root.is_absolute():raise _invalid()
        for folder in (root,root/'bin'):
            if folder.is_symlink() or not folder.is_dir():raise _invalid()
        path=root/'manifest.json'
        if path.is_symlink() or not path.is_file():raise _invalid()
        with path.open('rb') as stream:data=stream.read(1048577)
        if len(data)>1048576:raise _invalid()
        manifest=json.loads(data.decode('utf-8'))
        if not isinstance(manifest,dict) or manifest.get('schemaVersion')!=1 or not isinstance(manifest.get('version'),str) or not isinstance(manifest.get('files'),dict):raise _invalid()
        machine=platform.machine().lower()
        cpu={'amd64':'x86_64','x86_64':'x86_64','aarch64':'aarch64','arm64':'aarch64'}.get(machine)
        suffixes={'win32':['pc-windows-msvc'],'darwin':['apple-darwin'],'linux':['unknown-linux-gnu','unknown-linux-musl']}.get(sys.platform,[])
        if cpu is None or manifest.get('platform') not in [cpu+'-'+s for s in suffixes]:raise LocalAIError('PLATFORM_UNSUPPORTED')
        name='bin/originlm'+('.exe' if sys.platform=='win32' else '');record=manifest['files'].get(name)
        if not isinstance(record,dict) or type(record.get('bytes'))is not int or not 0<record['bytes']<=1073741824 or not re.fullmatch('[0-9a-f]{64}',str(record.get('sha256',''))):raise _invalid()
        binary=root/name;before=binary.lstat()
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode) or before.st_size!=record['bytes']:raise _invalid()
        with binary.open('rb')as stream:
            opened=os.fstat(stream.fileno())
            if (opened.st_ino,opened.st_dev)!=(before.st_ino,before.st_dev):raise _invalid()
            digest=hashlib.sha256();size=0
            for block in iter(lambda:stream.read(1048576),b''):
                size+=len(block)
                if size>record['bytes']:raise _invalid()
                digest.update(block)
            if size!=record['bytes'] or digest.hexdigest()!=record['sha256']:raise _invalid()
        return str(binary)
    except LocalAIError:raise
    except (OSError,ValueError,TypeError,OverflowError):raise _invalid() from None

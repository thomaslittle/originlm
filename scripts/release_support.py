"""Checksummed, deterministic local release bundles. Imports perform no I/O."""
from pathlib import Path, PurePosixPath
import hashlib
import re
import shutil
import stat
import zipfile

def _name(value):
    if not isinstance(value,str) or not value or len(value)>2048 or value.startswith('/') or '\\' in value or ':' in value or any(ord(c)<32 for c in value):
        raise ValueError('Invalid release entry')
    if any(part in ('','.','..') for part in value.split('/')) or PurePosixPath(value).is_absolute():
        raise ValueError('Invalid release entry')
    return value

def _files(root):
    root=Path(root)
    if root.is_symlink() or not root.is_dir():raise ValueError('Release directory is unavailable')
    items=[]
    for path in sorted(root.rglob('*')):
        if path.is_symlink():raise ValueError('Release entries cannot be links')
        if path.is_dir():continue
        if not path.is_file():raise ValueError('Release entries must be regular files')
        name=_name(path.relative_to(root).as_posix())
        if path.stat().st_size>1024*1024*1024:raise ValueError('Release entry is too large')
        items.append((name,path))
        if len(items)>10000:raise ValueError('Too many release entries')
    return items

def sha256(path):
    digest=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):digest.update(block)
    return digest.hexdigest()

def make_manifest(root,metadata):
    files={name:{'bytes':path.stat().st_size,'sha256':sha256(path)} for name,path in _files(root) if name!='manifest.json'}
    return {'schemaVersion':1,**metadata,'files':files}

def verify_manifest(root,manifest):
    if not isinstance(manifest,dict) or manifest.get('schemaVersion')!=1 or not isinstance(manifest.get('files'),dict):raise ValueError('Unsupported release manifest')
    files=manifest['files']
    for name,item in files.items():
        _name(name)
        if not isinstance(item,dict) or type(item.get('bytes')) is not int or item['bytes']<0 or not re.fullmatch('[0-9a-f]{64}',str(item.get('sha256',''))):raise ValueError('Invalid release checksum record')
    actual=make_manifest(root,{})['files']
    if actual!=files:raise ValueError('Release contents differ from the verified manifest')

def create_archive(root,destination):
    items=_files(root)
    if Path(destination).resolve().is_relative_to(Path(root).resolve()):raise ValueError('Archive must be outside its input directory')
    with zipfile.ZipFile(destination,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6,allowZip64=True) as archive:
        for name,path in items:
            info=zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0))
            info.create_system=3
            info.external_attr=(stat.S_IFREG | (path.stat().st_mode&0o777))<<16
            info.compress_type=zipfile.ZIP_DEFLATED
            with path.open('rb') as source,archive.open(info,'w',force_zip64=True) as target:shutil.copyfileobj(source,target,1024*1024)

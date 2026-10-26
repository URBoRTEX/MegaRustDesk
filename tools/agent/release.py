"""Local release signing and strict inventory checks. Never packages a launched app folder."""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import zipfile
from nacl.signing import SigningKey, VerifyKey

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / 'tools/agent/release_config.json'
KEY = ROOT / '.agent-build/signing/release-key.dpapi'
PREFIX = 'RustDesk-Agent-Portable-win-x64/'
ALLOWED_ROOT = {'RustDesk-Agent.exe','sciter.dll','manifest.json','source-state.json','ПРОЧИТАЙТЕ.md','Подключить MCP к Codex.cmd'}
def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()
def protect(data, encrypt):
    class Blob(ctypes.Structure):
        _fields_=[('size',wintypes.DWORD),('data',ctypes.POINTER(ctypes.c_ubyte))]
    source=(ctypes.c_ubyte*len(data)).from_buffer_copy(data)
    incoming=Blob(len(data),source); outgoing=Blob()
    api=ctypes.WinDLL('crypt32',use_last_error=True)
    function=api.CryptProtectData if encrypt else api.CryptUnprotectData
    function.argtypes=[ctypes.POINTER(Blob),ctypes.c_void_p,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_void_p,wintypes.DWORD,ctypes.POINTER(Blob)]
    function.restype=wintypes.BOOL
    if not function(ctypes.byref(incoming),None,None,None,None,1,ctypes.byref(outgoing)):
        raise RuntimeError('Windows could not protect/unlock the release signing key')
    try:
        return ctypes.string_at(outgoing.data,outgoing.size)
    finally:
        free=ctypes.WinDLL('kernel32').LocalFree;free.argtypes=[ctypes.c_void_p];free.restype=ctypes.c_void_p
        free(outgoing.data)
def load_key():
    value=KEY.read_bytes()
    if not value.startswith(b'MURD1'):
        raise RuntimeError('Unsupported protected signing-key format')
    return SigningKey(protect(value[5:],False))
def initialize():
    if CONFIG.exists():
        key=load_key()
        assert json.loads(CONFIG.read_text())['public_key']==key.verify_key.encode().hex()
        print('Existing protected signing key verified')
        return
    assert not KEY.exists(), 'Unexpected private key preserved'
    KEY.parent.mkdir(parents=True,exist_ok=True)
    key=SigningKey.generate()
    with KEY.open('xb') as stream:
        stream.write(b'MURD1'+protect(key.encode(),True))
    config={'version':'0.3.0','repository':'URBoRTEX/MegaRustDesk','public_key':key.verify_key.encode().hex()}
    CONFIG.write_text(json.dumps(config,indent=2)+'\n',encoding='utf-8')
    print('Protected signing key created; public key written to release_config.json')
def safe_name(name):
    if name in ALLOWED_ROOT:
        return
    if not name.startswith('licenses/') or not name.isascii() or len(name)>220 or '\\' in name or ':' in name:
        raise ValueError('Disallowed package path')
    for part in name.split('/'):
        lower=part.lower();base=lower.split('.')[0]
        if not part or part in {'.','..'} or part.endswith(('.', ' ')) or any(ord(c)<32 or c in '<>"|?*' for c in part) or base in {'con','prn','aux','nul'} or re.fullmatch(r'(?:com|lpt)[1-9]',base) or lower in {'data','runtime','auth.json','config.toml','history.jsonl','.env'} or lower.endswith(('.exe','.dll','.ps1','.cmd','.bat','.sqlite','.toml','.log','.png')):
            raise ValueError('Forbidden package file')
def strict_json(data):
    def pairs(items):
        result={}
        for key,value in items:
            if key in result: raise ValueError('Duplicate JSON key')
            result[key]=value
        return result
    return json.loads(data,object_pairs_hook=pairs)
def verify_package(path):
    path=Path(path)
    if path.stat().st_size>128*1024**2: raise ValueError('Package size limit exceeded')
    with zipfile.ZipFile(path) as archive:
        manifest_bytes=archive.read(PREFIX+'manifest.json')
        manifest=strict_json(manifest_bytes)
        config=json.loads(CONFIG.read_text())
        if manifest.get('product')!='RustDesk Agent Portable' or manifest.get('platform')!='windows-x64' or not re.fullmatch(r'\d+\.\d+\.\d+',manifest.get('version','')):
            raise ValueError('Wrong package product/platform/version')
        files=dict(manifest['files']); files['manifest.json']=hashlib.sha256(manifest_bytes).hexdigest()
        if not {'RustDesk-Agent.exe','sciter.dll','source-state.json','licenses/components.json','licenses/RustDesk-AGPL-3.0.txt'} <= files.keys():
            raise ValueError('Required portable payload is missing')
        if len(files)>4096 or len(archive.infolist())!=len(files): raise ValueError('Unexpected package inventory')
        seen=set(); total=0
        for info in archive.infolist():
            if not info.filename.startswith(PREFIX): raise ValueError('Wrong package root')
            name=info.filename[len(PREFIX):];safe_name(name)
            if info.is_dir() or name.casefold() in seen or (info.external_attr>>16)&0o170000==0o120000:
                raise ValueError('Link/duplicate/directory entry rejected')
            seen.add(name.casefold());total+=info.file_size
            if total>256*1024**2 or info.file_size>128*1024**2: raise ValueError('Expanded package limit exceeded')
            if name not in files or hashlib.sha256(archive.read(info)).hexdigest()!=files[name]:
                raise ValueError('Package hash/inventory mismatch')
        if manifest.get('embeddedCodexRuntime') is not False: raise ValueError('Embedded Codex rejected')
        return manifest,files
def require_release_source(manifest):
    from runtime_snapshot import verify
    if manifest.get('compiledRuntimeInputSha256')!=verify(): raise ValueError('Binary/runtime source snapshot mismatch')
    state=subprocess.check_output(['git','status','--porcelain','--untracked-files=all','--ignore-submodules=all'],cwd=ROOT,text=True)
    if state.strip(): raise ValueError('Commit the reviewed release sources before signing or creating source assets')
    head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    if manifest.get('sourceCommit')!=head: raise ValueError('Binary package must be built from the release commit')
    actual=subprocess.check_output(['git','-C','libs/hbb_common','diff','--binary'],cwd=ROOT)
    if actual.replace(b'\r\n',b'\n')!=(ROOT/'tools/agent/hbb-portable.patch').read_bytes().replace(b'\r\n',b'\n'):
        raise ValueError('hbb_common contains changes outside the published patch')
    return head
def prepare(path):
    config=json.loads(CONFIG.read_text());manifest,files=verify_package(path)
    version=config['version'];assert manifest['version']==version
    head=require_release_source(manifest)
    expected=f'MegaRustDesk-{version}-windows-x64.zip'
    assert Path(path).name==expected, 'Release binary asset must have a versioned name'
    tag='agent-v'+version
    payload={'schema':1,'product':'RustDesk Agent Portable','platform':'windows-x64','repository':config['repository'],
             'version':version,'tag':tag,'sourceCommit':head,
             'sourceBuildId':manifest['sourceBuildId'],'asset':{'name':expected,'url':f'https://github.com/{config["repository"]}/releases/download/{tag}/{expected}',
             'size':Path(path).stat().st_size,'sha256':sha(path)},'files':files}
    source=ROOT/'release'/f'MegaRustDesk-{version}-source.zip'
    if not source.is_file(): raise ValueError('Create the complete corresponding-source asset before signing')
    verify_source(source)
    payload['sourceAsset']={'name':source.name,'sha256':sha(source),'size':source.stat().st_size,'url':f'https://github.com/{config["repository"]}/releases/download/{tag}/{source.name}'}
    data=(json.dumps(payload,ensure_ascii=False,sort_keys=True,indent=2)+'\n').encode()
    key=load_key();assert key.verify_key.encode().hex()==config['public_key']
    signature=key.sign(data).signature
    VerifyKey(bytes.fromhex(config['public_key'])).verify(data,signature)
    folder=ROOT/'release'
    (folder/'update-manifest.json').write_bytes(data)
    (folder/'update-manifest.sig').write_text(signature.hex()+'\n',encoding='ascii')
    print(json.dumps({'version':version,'asset':expected,'sha256':sha(path),'signatureVerified':True,'inventoryCount':len(files)}))
def source_files():
    paths=subprocess.check_output(['git','ls-files','--recurse-submodules','-z'],cwd=ROOT).decode().split('\0')
    for required in ['src/agent/updater.rs','src/agent/update_install.rs','tools/agent/build.ps1','tools/agent/release.py','tools/agent/hbb-portable.patch','libs/hbb_common/src/config.rs']:
        if required not in paths: raise ValueError('Corresponding source is incomplete: '+required)
    result={}
    for name in sorted(set(paths)):
        if not name: continue
        parts=PurePosixPath(name).parts
        if any(part in {'.git','.agent-build','release','target','data','__pycache__'} for part in parts): raise ValueError('Private/generated path tracked in source tree')
        file=ROOT/name
        if file.is_file(): result['MegaRustDesk-source/'+name]=file
    return result
def verify_source(path):
    expected=source_files()
    with zipfile.ZipFile(path) as archive:
        if len(archive.infolist())!=len(expected) or set(archive.namelist())!=set(expected):
            raise ValueError('Corresponding-source inventory mismatch')
        for entry in archive.infolist():
            if entry.is_dir() or hashlib.sha256(archive.read(entry)).hexdigest()!=sha(expected[entry.filename]):
                raise ValueError('Corresponding-source file mismatch: '+entry.filename)
def source_archive():
    config=json.loads(CONFIG.read_text());path=ROOT/'release'/f'MegaRustDesk-{config["version"]}-source.zip'
    manifest,_=verify_package(ROOT/'release'/f'MegaRustDesk-{config["version"]}-windows-x64.zip')
    require_release_source(manifest)
    with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as output:
        for name,file in source_files().items(): output.write(file,name)
    verify_source(path)
    print(json.dumps({'sourceAsset':path.name,'sha256':sha(path)}))
def verify_prepared():
    config=json.loads(CONFIG.read_text());folder=ROOT/'release';version=config['version']
    data=(folder/'update-manifest.json').read_bytes()
    signature=bytes.fromhex((folder/'update-manifest.sig').read_text().strip())
    VerifyKey(bytes.fromhex(config['public_key'])).verify(data,signature)
    payload=strict_json(data)
    if payload['repository']!=config['repository'] or payload['version']!=version or payload['tag']!='agent-v'+version:
        raise ValueError('Prepared release identity mismatch')
    manifest,files=verify_package(folder/payload['asset']['name'])
    require_release_source(manifest)
    verify_source(folder/payload['sourceAsset']['name'])
    if payload['files']!=files or payload['sourceCommit']!=manifest['sourceCommit'] or payload['sourceBuildId']!=manifest['sourceBuildId']:
        raise ValueError('Signed inventory/source identity mismatch')
    paths=[]
    for field,suffix in [('asset','windows-x64'),('sourceAsset','source')]:
        asset=payload[field];name=f'MegaRustDesk-{version}-{suffix}.zip';path=folder/name
        if asset['name']!=name or asset['url']!=f'https://github.com/{config["repository"]}/releases/download/{payload["tag"]}/{name}' or asset['size']!=path.stat().st_size or asset['sha256']!=sha(path):
            raise ValueError('Prepared release asset mismatch')
        paths.append(path)
    paths.extend([folder/'update-manifest.json',folder/'update-manifest.sig'])
    return payload,paths
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('action',choices=['init','verify','prepare','source','verify-prepared']);parser.add_argument('--archive',type=Path)
    args=parser.parse_args()
    if args.action=='init': initialize()
    elif args.action=='source': source_archive()
    elif args.action=='prepare': prepare(args.archive)
    elif args.action=='verify-prepared':
        payload,paths=verify_prepared();print(json.dumps({'version':payload['version'],'signatureVerified':True,'assetsVerified':len(paths)}))
    else:
        manifest,files=verify_package(args.archive);print(json.dumps({'version':manifest['version'],'inventoryCount':len(files),'verified':True}))

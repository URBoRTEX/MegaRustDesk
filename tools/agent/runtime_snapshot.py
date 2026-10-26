"""Bind a locally compiled binary to unchanged source inputs across packaging/commit."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
ROOT=Path(__file__).resolve().parents[2]
SNAPSHOT=ROOT/'.agent-build/runtime-snapshot.json'
def sha(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
def inputs():
    paths=set()
    for flags in [['--cached','--others','--exclude-standard'],['--recurse-submodules']]:
        paths.update(subprocess.check_output(['git','ls-files','-z']+flags,cwd=ROOT).decode().split('\0'))
    paths.update(['src/ui/inline.rs','tools/agent/release_config.json'])
    return sorted(n for n in paths if n and n!='src/version.rs' and (n.startswith(('src/','libs/','res/')) or n in {'Cargo.toml','Cargo.lock','build.rs','tools/agent/release_config.json'}) and (ROOT/n).is_file())
def capture():
    files={n:sha(ROOT/n) for n in inputs()}
    SNAPSHOT.parent.mkdir(exist_ok=True)
    SNAPSHOT.write_text(json.dumps({'files':files,'digest':hashlib.sha256(json.dumps(files,sort_keys=True).encode()).hexdigest()},indent=2),encoding='utf-8')
def verify():
    saved=json.loads(SNAPSHOT.read_text())
    current={n:sha(ROOT/n) for n in inputs()}
    if saved['files']!=current:raise ValueError('Runtime source changed after compilation; rebuild before packaging/signing')
    return saved['digest']
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('action',choices=['capture','verify']);args=parser.parse_args()
    if args.action=='capture':capture();print('Runtime source snapshot captured')
    else:print('Runtime source snapshot verified',verify())

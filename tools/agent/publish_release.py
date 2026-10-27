"""Publish immutable assets to the updater channel, with explicit preview opt-in."""
import argparse
import json
import subprocess
import tempfile
from pathlib import Path
from release import ROOT, sha, verify_prepared

def gh(*args):
    return subprocess.check_output(['gh',*map(str,args)],cwd=ROOT,text=True,encoding='utf-8').strip()

def publish(notes, publish_now, preview=False):
    payload,assets=verify_prepared()
    repo=payload['repository'];tag=payload['tag']
    metadata=json.loads(gh('api','repos/'+repo))
    if metadata['full_name']!=repo or metadata['private'] or not metadata['fork'] or metadata['parent']['full_name']!='rustdesk/rustdesk':
        raise ValueError('Expected public RustDesk fork only')
    remote_tag=gh('api',f'repos/{repo}/git/ref/tags/{tag}')
    target=json.loads(remote_tag)['object']
    if target['type']=='tag':
        target=json.loads(gh('api',f'repos/{repo}/git/tags/{target["sha"]}'))['object']
    if target['type']!='commit' or target['sha']!=payload['sourceCommit']:
        raise ValueError('Remote release tag does not match reviewed source commit')
    # This reviewed source is also covered by the signed corresponding-source ZIP.
    repair=ROOT/'tools/agent/repair-update.cmd'
    if repair.is_file():
        if repair.is_symlink(): raise ValueError('Repair script must be a plain tracked source file')
        assets.append(repair)
    sums=ROOT/'release/SHA256SUMS.txt'
    sums.write_text(''.join(f'{sha(p)}  {p.name}\n' for p in assets),encoding='ascii')
    assets.append(sums)
    releases=json.loads(gh('release','list','--repo',repo,'--limit','100','--json','tagName,isDraft'))
    existing=next((r for r in releases if r['tagName']==tag),None)
    if existing and not existing['isDraft']:
        raise ValueError('Published releases are immutable; use a new version')
    prerelease='--prerelease='+str(preview).lower()
    title=f'MegaRustDesk {payload["version"]}'+(' preview' if preview else '')
    if not existing:
        gh('release','create',tag,'--repo',repo,'--verify-tag','--draft',prerelease,'--title',title,'--notes-file',notes)
    release=json.loads(gh('release','view',tag,'--repo',repo,'--json','assets,isDraft'))
    remote_names={a['name'] for a in release['assets']}
    if remote_names-set(p.name for p in assets):
        raise ValueError('Draft contains unexpected assets; inspect manually')
    for path in assets:
        if path.name not in remote_names:
            gh('release','upload',tag,path,'--repo',repo)
    # Verify every stored asset before changing draft visibility. No --clobber.
    with tempfile.TemporaryDirectory(prefix='megarustdesk-release-') as folder:
        gh('release','download',tag,'--repo',repo,'--dir',folder)
        for path in assets:
            if sha(Path(folder)/path.name)!=sha(path):
                raise ValueError('Remote uploaded asset differs: '+path.name)
    verify_prepared()
    if publish_now:
        args=['release','edit',tag,'--repo',repo,'--draft=false',prerelease,'--title',title]
        if not preview: args.append('--latest')
        gh(*args)
    print(json.dumps({'repository':repo,'tag':tag,'published':publish_now,'publishedPreview':publish_now and preview,'channel':'preview' if preview else 'updater','remoteAssetsVerified':len(assets)}))

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--notes-file',type=Path,required=True)
    parser.add_argument('--publish',action='store_true')
    parser.add_argument('--preview',action='store_true',help='Exclude this prerelease from the installed client update button')
    args=parser.parse_args()
    publish(args.notes_file.resolve(),args.publish,args.preview)

"""Package an actual compiled exe using an explicit allowlist; never copies runtime data."""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import zipfile
from release import safe_name, verify_package
from runtime_snapshot import verify as verify_runtime_source

ROOT = pathlib.Path(__file__).resolve().parents[2]

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--build-root', type=pathlib.Path, required=True)
    args = parser.parse_args()
    release_config = json.loads((ROOT / 'tools/agent/release_config.json').read_text())
    runtime_digest=verify_runtime_source()
    executable = args.build_root / 'target/release/rustdesk.exe'
    if not executable.is_file():
        raise SystemExit('No compiled Windows executable; refusing to produce a placeholder release')
    # Build in private staging: a user may already have launched the unpacked
    # release directory, so its settings/credentials must never be overwritten.
    destination = ROOT / '.agent-build/mcp-package/RustDesk-Agent-Portable-win-x64'
    destination.mkdir(parents=True, exist_ok=True)
    if (destination / 'runtime').exists():
        raise SystemExit('Unexpected embedded runtime in external-MCP staging; preserved for inspection')
    # Reuse one staging. Refuse to package any directory where user data has appeared.
    if (destination / 'data').exists():
        raise SystemExit('Staging contains runtime/user data; use the preserved source to investigate')
    shutil.copy2(executable, destination / 'RustDesk-Agent.exe')
    subprocess.run([sys.executable, str(ROOT / 'tools/agent/make_diagnostic.py')], check=True)
    shutil.copy2(ROOT / 'release/Проверить соединение.cmd', destination / 'Проверить соединение.cmd')
    shutil.copy2(args.build_root / 'toolchain/sciter.dll', destination / 'sciter.dll')
    inputs = json.loads((ROOT / 'tools/agent/inputs.json').read_text())
    assert sha(destination / 'sciter.dll') == inputs['sciter']['sha256'], 'Sciter runtime hash mismatch'
    licenses = destination / 'licenses'
    licenses.mkdir(exist_ok=True)
    shutil.copy2(ROOT / 'LICENCE', licenses / 'RustDesk-AGPL-3.0.txt')
    for name in ['sciter-license.htm']:
        source = args.build_root / 'toolchain' / name
        if not source.is_file():
            raise SystemExit('Missing required license notice: ' + name)
        shutil.copy2(source, licenses / name)
    installed = args.build_root / 'toolchain/vcpkg/installed/x64-windows-static/share'
    for directory in installed.iterdir():
        copyright_file = directory / 'copyright'
        if copyright_file.is_file():
            shutil.copy2(copyright_file, licenses / (directory.name + '.txt'))
    metadata = json.loads(subprocess.check_output(['cargo', '+1.75.0', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], cwd=ROOT))
    notices = []
    for package in metadata['packages']:
        notices.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'), 'repository': package.get('repository')})
        folder = pathlib.Path(package['manifest_path']).parent
        for source in folder.iterdir():
            if source.is_file() and source.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')):
                target = licenses / (package['name'] + '-' + package['version'])
                target.mkdir(exist_ok=True)
                shutil.copy2(source, target / source.name)
    (licenses / 'components.json').write_text(json.dumps(notices, indent=2), encoding='utf-8')
    shutil.copy2(ROOT / 'docs/agent/QUICKSTART.md', destination / 'ПРОЧИТАЙТЕ.md')
    shutil.copy2(ROOT / 'tools/agent/connect-codex.cmd', destination / 'Подключить MCP к Codex.cmd')
    tracked = subprocess.check_output(['git', '-c', 'core.quotepath=false', 'ls-files', '--cached', '--others', '--exclude-standard'], cwd=ROOT, text=True, encoding='utf-8').splitlines()
    source_files = sorted({name for name in tracked + ['Cargo.lock', 'src/ui/inline.rs', 'src/version.rs'] if not name.startswith('docs/') and '__pycache__' not in name and (ROOT / name).is_file()})
    source_state = {'upstream': inputs['rustdesk']['commit'], 'head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(), 'hbbCommonCommit': subprocess.check_output(['git','-C','libs/hbb_common','rev-parse','HEAD'],cwd=ROOT,text=True).strip(), 'hbbPatchSha256': sha(ROOT/'tools/agent/hbb-portable.patch'), 'lockSha256': sha(ROOT/'Cargo.lock'), 'inputsSha256': sha(ROOT/'tools/agent/inputs.json'), 'files': {name: sha(ROOT / name) for name in source_files}}
    hbb_files=subprocess.check_output(['git','-C','libs/hbb_common','ls-files','-z'],cwd=ROOT).decode().split('\0')
    source_state['hbbFiles']={name:sha(ROOT/'libs/hbb_common'/name) for name in hbb_files if name and (ROOT/'libs/hbb_common'/name).is_file()}
    source_id = hashlib.sha256(json.dumps(source_state, sort_keys=True).encode()).hexdigest()
    source_state['sourceBuildId'] = source_id
    (destination / 'source-state.json').write_text(json.dumps(source_state, indent=2), encoding='utf-8')
    files = sorted(path for path in destination.rglob('*') if path.is_file() and path != destination / 'manifest.json')
    for path in files:
        safe_name(path.relative_to(destination).as_posix())
        assert not path.is_symlink() and not path.is_junction()
    forbidden = {'auth.json', 'config.toml', '.env', 'history.jsonl', 'state_5.sqlite'}
    assert not any(path.name in forbidden or 'data' in path.relative_to(destination).parts for path in files)
    manifest = {
        'product': 'RustDesk Agent Portable', 'version': release_config['version'], 'platform': 'windows-x64',
        'rustdeskUpstream': '6a0da9cf09b80bba6ca731d3029210c5d839a77b',
        'branch': subprocess.check_output(['git', 'branch', '--show-current'], cwd=ROOT, text=True).strip(),
        'codex': 'external-cli-or-mcp-client', 'embeddedCodexRuntime': False, 'buildCompiled': True, 'buildVerified': False, 'acceptanceStatus': 'UNVERIFIED',
        'buildInputs': inputs,
        'sourceBuildId': source_id,
        'sourceCommit': source_state['head'],
        'compiledRuntimeInputSha256': runtime_digest,
        'networkPolicy': 'outbound-tcp-relay-only',
        'files': {str(path.relative_to(destination)).replace('\\', '/'): sha(path) for path in files},
    }
    (destination / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    archive = ROOT / ('release/MegaRustDesk-' + release_config['version'] + '-windows-x64.zip')
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as output:
        for path in sorted(destination.rglob('*')):
            if path.is_file():
                output.write(path, destination.name + '/' + str(path.relative_to(destination)))
    verify_package(archive)
    (archive.parent / 'SHA256SUMS.txt').write_text(sha(archive) + '  ' + archive.name + '\n')
    print(str(archive))
    print('SHA256', sha(archive))
    print('Build produced; acceptance remains UNVERIFIED until package/receiver/agent checks pass')

if __name__ == '__main__':
    main()

"""Verify the built archive in one relocatable QA directory, without developer PATH."""
import hashlib
import argparse
import json
import os
import pathlib
import subprocess
import zipfile
import winreg
from release import verify_package as verify_inventory

ROOT = pathlib.Path(__file__).resolve().parents[2]
QA = pathlib.Path('C:/CodexBuilds/MegaRustDesk/qa-mcp')
REPORT = ROOT / '.agent-build/package-check.json'

def digest(path):
    return hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()

def path_hash():
    result = {}
    for label, hive, key in [('user', winreg.HKEY_CURRENT_USER, 'Environment'), ('machine', winreg.HKEY_LOCAL_MACHINE, r'SYSTEM\CurrentControlSet\Control\Session Manager\Environment')]:
        with winreg.OpenKey(hive, key) as handle:
            value = winreg.QueryValueEx(handle, 'Path')[0]
        result[label] = hashlib.sha256(value.encode()).hexdigest()
    return result

def codex_metadata():
    result = {}
    for name in ['config.toml', 'auth.json']:
        path = pathlib.Path.home() / '.codex' / name
        result[name] = [path.stat().st_size, path.stat().st_mtime_ns] if path.exists() else None
    return result

def run(folder):
    executable = folder / 'RustDesk-Agent-Portable-win-x64/RustDesk-Agent.exe'
    env = dict(os.environ)
    for name in ['CODEX_HOME', 'OPENAI_API_KEY', 'CODEX_API_KEY', 'AZURE_OPENAI_API_KEY', 'ANTHROPIC_API_KEY', 'NODE_PATH', 'PYTHONPATH', 'CARGO_HOME', 'RUSTUP_HOME', 'VCPKG_ROOT', 'LIBCLANG_PATH']:
        env.pop(name, None)
    env['PATH'] = os.environ['SystemRoot'] + r'\System32;' + os.environ['SystemRoot']
    process = subprocess.run([str(executable), '--agent-self-test'], cwd=folder, env=env, capture_output=True, text=True, encoding='utf-8', timeout=120, creationflags=subprocess.CREATE_NO_WINDOW)
    print('SELFTEST', str(folder), 'exit', process.returncode, flush=True)
    if process.returncode:
        print(process.stdout[-2000:], process.stderr[-2000:], flush=True)
        return {'path': str(folder), 'exitCode': process.returncode, 'error': process.stderr[-2000:]}
    value = json.loads(process.stdout.strip().splitlines()[-1])
    if value.get('outboundRelayOnly') is not True:
        return {'path': str(folder), 'exitCode': 1, 'error': 'Outbound relay policy not confirmed', **value}
    return {'path': str(folder), 'exitCode': 0, **value}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--reuse-qa', action='store_true', help='Validate and refresh this task-owned QA payload; preserve its data')
    parser.add_argument('--archive', type=pathlib.Path)
    args = parser.parse_args()
    QA.mkdir(parents=True, exist_ok=True)
    first = QA / 'Portable Test'
    second = QA / 'Проверка переноса'
    if second.exists():
        if not args.reuse_qa or first.exists():
            raise SystemExit('Relocated QA directory already exists; inspect it before reuse')
        # Explicit task-owned reuse: verify all previous payload hashes before moving.
        package = second / 'RustDesk-Agent-Portable-win-x64'
        old = json.loads((package / 'manifest.json').read_text(encoding='utf-8'))
        for relative, expected in old['files'].items():
            target = (package / relative).resolve()
            assert target.is_relative_to(package.resolve()), relative
            assert digest(target) == expected, 'Modified QA payload: ' + relative
        allowed = set(old['files']) | {'manifest.json'}
        for target in second.rglob('*'):
            if target.is_symlink() or target.is_junction():
                raise SystemExit('QA link preserved; inspect before reuse')
            if target.is_file():
                relative = target.relative_to(package).as_posix()
                if relative not in allowed and not relative.startswith('data/'):
                    raise SystemExit('Unexpected QA file preserved: ' + relative)
        assert first.resolve().parent == QA.resolve() and second.resolve().parent == QA.resolve()
        second.rename(first)
    before_path, before_codex = path_hash(), codex_metadata()
    config = json.loads((ROOT / 'tools/agent/release_config.json').read_text())
    archive = args.archive or ROOT / ('release/MegaRustDesk-' + config['version'] + '-windows-x64.zip')
    verify_inventory(archive)
    first.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as source:
        if any('/runtime/' in name for name in source.namelist()):
            raise SystemExit('Unexpected embedded runtime in external-MCP distribution')
        # Reuse only the exact unlaunched extraction from a failed manifest check.
        allowed = set(source.namelist())
        existing = [path for path in first.rglob('*') if path.is_file()]
        data_prefix = 'RustDesk-Agent-Portable-win-x64/data/'
        if any(path.relative_to(first).as_posix() not in allowed and not (args.reuse_qa and path.relative_to(first).as_posix().startswith(data_prefix)) for path in existing):
            raise SystemExit('Existing QA folder contains files outside this archive; preserved')
        for name in source.namelist():
            if not (first / name).resolve().is_relative_to(first.resolve()):
                raise SystemExit('Unsafe archive entry')
        source.extractall(first)
    package = first / 'RustDesk-Agent-Portable-win-x64'
    manifest = json.loads((package / 'manifest.json').read_text(encoding='utf-8'))
    for relative, expected in manifest['files'].items():
        assert digest(package / relative) == expected, relative
    results = [run(first)]
    # Both absolute paths are validated within this task's QA root before moving.
    assert first.resolve().parent == QA.resolve() and second.resolve().parent == QA.resolve()
    first.rename(second)
    results.append(run(second))
    report = {'archiveSha256': digest(archive), 'manifestHashesVerified': True, 'runs': results, 'systemPathUnchanged': path_hash() == before_path, 'globalCodexFilesMetadataUnchanged': codex_metadata() == before_codex, 'cleanWindowsVmTested': False, 'remoteGuiTaskTested': False}
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
    if any(row['exitCode'] != 0 for row in results):
        raise SystemExit(1)

if __name__ == '__main__':
    main()

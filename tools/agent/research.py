"""Fetch only public build inputs/documentation; never reads user configuration."""
import concurrent.futures
import html
import json
import pathlib
import re
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / '.agent-build' / 'research'
OUT.mkdir(parents=True, exist_ok=True)
URLS = {
    'codex-app-server': 'https://developers.openai.com/codex/app-server',
    'codex-mcp': 'https://developers.openai.com/codex/mcp',
    'codex-auth': 'https://developers.openai.com/codex/auth',
    'rustdesk-build': 'https://rustdesk.com/docs/en/dev/build/windows/',
    'rustdesk-elevation': 'https://rustdesk.com/docs/en/client/windows/windows-portable-elevation/',
    'rustdesk-releases': 'https://api.github.com/repos/rustdesk/rustdesk/releases/tags/1.4.3',
    'codex-releases': 'https://api.github.com/repos/openai/codex/releases/tags/rust-v0.153.4',
    'vcpkg-releases': 'https://api.github.com/repos/rustdesk/vcpkg/releases',
}

def fetch(item):
    name, url = item
    try:
        req = urllib.request.Request(url, headers={'User-Agent': 'RustDesk-Agent-build'})
        data = urllib.request.urlopen(req, timeout=90).read().decode()
        if 'api.github.com' in url:
            obj = json.loads(data)
            items = obj if isinstance(obj, list) else [obj]
            for release in items[:4]:
                print(name, release.get('tag_name'), [(a['name'], a['size']) for a in release.get('assets', [])])
        else:
            data = re.sub(r'<script[\s\S]*?</script>|<style[\s\S]*?</style>', '', data)
            data = html.unescape(re.sub(r'<[^>]+>', ' ', data))
            data = re.sub(r'[ \t]+', ' ', data)
            print(name, len(data))
        (OUT / (name + '.txt')).write_text(data, encoding='utf-8')
    except Exception as exc:
        print(name, type(exc).__name__, str(exc))

with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
    list(pool.map(fetch, URLS.items()))

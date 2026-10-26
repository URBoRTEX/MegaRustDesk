"""Check shipped official App Server transport/config with isolated, unauthenticated data.

No login, model turn, API billing or real user configuration is used.
"""
import json
import os
import pathlib
import queue
import subprocess
import threading

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / '.agent-build' / 'codex-smoke'
OUT.mkdir(parents=True, exist_ok=True)
runtime = pathlib.Path('C:/CodexBuilds/MegaRustDesk/toolchain/codex-package/bin/codex.exe')
arguments = [str(runtime), 'app-server', '--listen', 'stdio://']
for config in json.loads((ROOT / 'src/agent/codex-config.json').read_text()):
    arguments += ['-c', config]
env = {k: v for k, v in os.environ.items() if k not in ('OPENAI_API_KEY', 'AZURE_OPENAI_API_KEY', 'ANTHROPIC_API_KEY')}
env['CODEX_HOME'] = str(OUT / 'home')
pathlib.Path(env['CODEX_HOME']).mkdir(exist_ok=True)
process = subprocess.Popen(arguments, env=env, cwd=OUT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
messages = queue.Queue()
threading.Thread(target=lambda: [messages.put(json.loads(line)) for line in process.stdout], daemon=True).start()

def rpc(identifier, method, params):
    process.stdin.write(json.dumps({'id': identifier, 'method': method, 'params': params}) + '\n')
    process.stdin.flush()
    while True:
        reply = messages.get(timeout=60)
        if reply.get('id') == identifier:
            assert 'error' not in reply, reply
            return reply['result']

try:
    initialized = rpc(1, 'initialize', {'clientInfo': {'name': 'rustdesk_agent_portable_test', 'version': '0.1.0'}, 'capabilities': {}})
    process.stdin.write('{"method":"initialized","params":{}}\n')
    process.stdin.flush()
    account = rpc(2, 'account/read', {'refreshToken': False})
    assert account.get('account') is None, 'Test must not use a real account'
    result = {'initialize': True, 'accountRead': True, 'isolatedUnauthenticatedHome': True, 'modelTurnTested': False, 'userAgent': initialized.get('userAgent')}
    (OUT / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(result, ensure_ascii=False))
finally:
    process.terminate()
    process.wait(timeout=10)

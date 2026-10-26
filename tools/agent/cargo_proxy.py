"""Task-local verified HTTPS transport for Cargo's Windows Schannel incompatibility.

No TLS checks are disabled. Cargo validates crate hashes against its lockfile.
Only the two official crates.io origins are reachable through this loopback server.
"""
import argparse
import http.server
import json
import pathlib
import time
import urllib.request
import urllib.error

CACHE = pathlib.Path('C:/CodexBuilds/MegaRustDesk/toolchain/registry-index-cache')

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path == '/index/config.json':
            body = json.dumps({'dl': f'http://127.0.0.1:{self.server.server_port}/crates', 'api': 'https://crates.io'}).encode()
        elif self.path.startswith('/index/'):
            return self.forward('https://index.crates.io/' + self.path[len('/index/'):])
        elif self.path.startswith('/crates/'):
            parts = self.path.split('/')
            if len(parts) != 5 or parts[4] != 'download':
                return self.send_error(404)
            return self.forward(f'https://static.crates.io/crates/{parts[2]}/{parts[2]}-{parts[3]}.crate')
        else:
            return self.send_error(404)
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def forward(self, url):
        if '..' in self.path or '?' in self.path or '%' in self.path:
            return self.send_error(400)
        cached = CACHE / self.path.removeprefix('/index/') if self.path.startswith('/index/') else None
        for attempt in range(3):
            try:
                if cached is not None and cached.is_file():
                    body = cached.read_bytes()
                else:
                    with urllib.request.urlopen(url, timeout=20) as response:
                        body = response.read()
                    if cached is not None:
                        cached.parent.mkdir(parents=True, exist_ok=True)
                        cached.write_bytes(body)
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            except Exception as exc:
                if isinstance(exc, urllib.error.HTTPError) and exc.code == 404:
                    return self.send_error(404)
                if attempt == 2:
                    print(self.path, type(exc).__name__, str(exc), flush=True)
                    return self.send_error(502, str(exc))
                time.sleep(1)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=19473)
    args = parser.parse_args()
    http.server.ThreadingHTTPServer(('127.0.0.1', args.port), Handler).serve_forever()

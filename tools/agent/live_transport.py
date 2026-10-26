"""Temporary same-session transport repair while the old viewer stays open.

No desktop APIs, clipboard, permission changes, credential decryption or input retries.
The portable 0.2.3 EXE contains the permanent fix without a Python dependency.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import socket
import time

REQUEST_LIMIT = 1024 * 1024
RESPONSE_LIMIT = 24 * 1024 * 1024
READ_ONLY = {'screenshot', 'session_status'}


def receive(stream, limit):
    message = bytearray()
    while not message.endswith(b'\n'):
        chunk = stream.recv(min(65536, limit + 1 - len(message)))
        if not chunk:
            raise EOFError('incomplete loopback message')
        message.extend(chunk)
        if len(message) > limit:
            raise ValueError('loopback message size limit')
    return bytes(message)


def parse_request(request):
    try:
        value = json.loads(request)
    except RecursionError as error:
        raise ValueError('MCP request nesting exceeds parser limit') from error
    if not isinstance(value, dict) or not isinstance(value.get('name'), str):
        raise ValueError('MCP request must be an object with a string tool name')
    return value


def forward(address, request):
    value = parse_request(request)
    attempts = 3 if value.get('name') in READ_ONLY else 1
    for attempt in range(attempts):
        try:
            with socket.create_connection(address, timeout=3) as stream:
                stream.settimeout(10)
                stream.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                stream.sendall(request)
                response = receive(stream, RESPONSE_LIMIT)
                json.loads(response)  # Complete application errors pass through once.
                return response, attempt + 1
        except (EOFError, ConnectionAbortedError, ConnectionResetError,
                BrokenPipeError, TimeoutError):
            if attempt + 1 == attempts:
                raise
            time.sleep(0.04 * (attempt + 1))


def alive(pid):
    kernel = ctypes.windll.kernel32
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    handle = kernel.OpenProcess(0x1000, False, pid)
    if not handle:
        return False
    try:
        code = ctypes.c_ulong()
        return bool(kernel.GetExitCodeProcess(handle, ctypes.byref(code))) and code.value == 259
    finally:
        kernel.CloseHandle(handle)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--endpoint', type=Path, required=True)
    parser.add_argument('--session-id', required=True)
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    path = args.endpoint.resolve(strict=True)
    original = path.read_bytes()
    endpoint = json.loads(original)
    if endpoint['session_id'] != args.session_id or not alive(endpoint['pid']):
        raise SystemExit('The selected viewer session is not running')
    receipt = {'active': False, 'requests': 0, 'recoveredReads': 0,
               'failedRequests': 0, 'inputRetries': 0, 'permissionChanges': 0,
               'sameSessionOnly': True, 'pid': os.getpid()}

    def save():
        args.receipt.write_text(json.dumps(receipt, indent=2), encoding='utf-8')

    listener = socket.socket()
    listener.bind(('127.0.0.1', 0))
    listener.listen()
    listener.settimeout(0.5)
    replacement = json.dumps({**endpoint, 'port': listener.getsockname()[1]}).encode()
    temporary = path.with_suffix('.relay.tmp')
    if temporary.exists():
        raise SystemExit('Unexpected relay temporary file; preserved')
    stop = args.receipt.with_suffix('.stop')
    try:
        temporary.write_bytes(replacement)
        os.replace(temporary, path)
        receipt['active'] = True
        save()
        while alive(endpoint['pid']) and not stop.exists():
            if not path.exists() or path.read_bytes() != replacement:
                break  # A new viewer endpoint must never be overwritten or followed.
            try:
                client, _ = listener.accept()
            except socket.timeout:
                continue
            with client:
                client.settimeout(15)
                try:
                    request = receive(client, REQUEST_LIMIT)
                    value = parse_request(request)
                    if value.get('session_id') != args.session_id:
                        raise ValueError('Different target rejected')
                    response, attempts = forward(('127.0.0.1', endpoint['port']), request)
                    if attempts > 1:
                        receipt['recoveredReads'] += 1
                    receipt['requests'] += 1
                    client.sendall(response)
                except (OSError, EOFError, ValueError):
                    receipt['failedRequests'] += 1
                    response = json.dumps({'isError': True, 'content': [{'type': 'text', 'text':
                        'MCP transport interrupted; input was not repeated. Check session_status before continuing.'}]})
                    try:
                        client.sendall((response + '\n').encode())
                    except OSError:
                        pass
                save()
    finally:
        listener.close()
        if path.exists() and path.read_bytes() == replacement:
            path.write_bytes(original)
            receipt['originalEndpointRestored'] = True
        receipt['active'] = False
        save()


if __name__ == '__main__':
    main()

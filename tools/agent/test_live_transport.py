"""Synthetic loopback regressions; never connects to a RustDesk session."""
import json
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from live_transport import forward, receive


class TransportTests(unittest.TestCase):
    def test_non_object_request_is_rejected_without_connecting(self):
        for request in [b'null\n', b'[]\n', b'"text"\n', b'{"name":[]}\n', b'[' * 20000 + b']' * 20000 + b'\n']:
            with self.assertRaises(ValueError):
                forward(('127.0.0.1', 0), request)

    def run_server(self, name, failures):
        listener = socket.socket()
        listener.bind(('127.0.0.1', 0))
        listener.listen()
        address = listener.getsockname()
        calls = []
        count = failures + 1 if name == 'screenshot' else 1

        def serve():
            with listener:
                for i in range(count):
                    client, _ = listener.accept()
                    with client:
                        calls.append(receive(client, 1024))
                        if i < failures:
                            client.sendall(b'{"content":[')
                        else:
                            client.sendall(b'{"content":[]}\n')

        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        request = (json.dumps({'name': name, 'session_id': 'synthetic', 'token': 'synthetic'}) + '\n').encode()
        try:
            response, attempts = forward(address, request)
            self.assertEqual(response, b'{"content":[]}\n')
            self.assertEqual(attempts, count)
        finally:
            worker.join(timeout=3)
        self.assertEqual(calls, [request] * count)

    def test_recovers_two_truncated_screenshots_without_changing_request(self):
        self.run_server('screenshot', 2)

    def test_does_not_replay_input_after_truncated_reply(self):
        with self.assertRaises(EOFError):
            self.run_server('type_text', 1)

    def test_complete_application_error_is_not_retried(self):
        listener = socket.socket()
        listener.bind(('127.0.0.1', 0))
        listener.listen()
        address = listener.getsockname()
        def serve():
            with listener:
                client, _ = listener.accept()
                with client:
                    receive(client, 1024)
                    client.sendall(b'{"isError":true}\n')
        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        response, attempts = forward(address, b'{"name":"screenshot"}\n')
        self.assertEqual(json.loads(response), {'isError': True})
        self.assertEqual(attempts, 1)
        worker.join(timeout=3)

    @unittest.skipUnless(sys.platform == 'win32', 'Windows process-liveness API')
    def test_invalid_client_does_not_kill_relay_and_endpoint_is_restored(self):
        import os
        listener = socket.socket()
        listener.bind(('127.0.0.1', 0))
        listener.listen()
        def serve():
            with listener:
                client, _ = listener.accept()
                with client:
                    receive(client, 1024)
                    client.sendall(b'{"content":[]}\n')
        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        with tempfile.TemporaryDirectory(prefix='megarustdesk-relay-test-') as folder:
            root = Path(folder)
            endpoint, receipt = root / 'endpoint.json', root / 'receipt.json'
            original = json.dumps(dict(session_id='synthetic', pid=os.getpid(),
                                       port=listener.getsockname()[1], protected_token='synthetic')).encode()
            endpoint.write_bytes(original)
            process = subprocess.Popen([sys.executable, str(Path(__file__).with_name('live_transport.py')),
                '--endpoint', str(endpoint), '--session-id', 'synthetic', '--receipt', str(receipt)],
                creationflags=subprocess.CREATE_NO_WINDOW, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                deadline = time.monotonic() + 5
                while not receipt.exists() and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(receipt.exists())
                address = ('127.0.0.1', json.loads(endpoint.read_bytes())['port'])
                for request in [b'[]\n', b'[' * 20000 + b']' * 20000 + b'\n', b'{"session_id":"synthetic","name":[]}\n',
                                b'{"session_id":"other","name":"session_status"}\n']:
                    with socket.create_connection(address, timeout=3) as client:
                        client.sendall(request)
                        self.assertTrue(json.loads(receive(client, 1024))['isError'])
                with socket.create_connection(address, timeout=3) as client:
                    client.sendall(b'{"session_id":"synthetic","name":"session_status"}\n')
                    self.assertEqual(json.loads(receive(client, 1024)), {'content': []})
            finally:
                receipt.with_suffix('.stop').touch()
                process.communicate(timeout=5)
            self.assertEqual(process.returncode, 0)
            self.assertEqual(endpoint.read_bytes(), original)
            self.assertFalse(json.loads(receipt.read_text())['active'])
        worker.join(timeout=3)

    def test_oversized_message_is_rejected(self):
        left, right = socket.socketpair()
        with left, right:
            left.sendall(b'123456\n')
            with self.assertRaises(ValueError):
                receive(right, 5)


if __name__ == '__main__':
    unittest.main()

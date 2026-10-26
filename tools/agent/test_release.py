"""Producer guards use synthetic packages, never real device data/signing keys."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import zipfile
import release

class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='megarustdesk-producer-')
        self.folder=Path(self.temp.name)
        self.archive=self.folder/'package.zip'
    def tearDown(self): self.temp.cleanup()
    def package(self, extra=None, changed=False, omit=None):
        files={name:b'synthetic' for name in ['RustDesk-Agent.exe','sciter.dll','source-state.json','licenses/components.json','licenses/RustDesk-AGPL-3.0.txt']}
        if omit: del files[omit]
        manifest={'product':'RustDesk Agent Portable','platform':'windows-x64','version':'0.3.0','embeddedCodexRuntime':False,'files':{n:hashlib.sha256(b).hexdigest() for n,b in files.items()}}
        files['manifest.json']=json.dumps(manifest).encode()
        if extra: files[extra]=b'forbidden'
        if changed: files['RustDesk-Agent.exe']=b'tampered'
        with zipfile.ZipFile(self.archive,'w') as archive:
            for n,b in files.items(): archive.writestr(release.PREFIX+n,b)
        return self.archive
    def test_clean_package(self):
        manifest,files=release.verify_package(self.package())
        self.assertEqual(manifest['version'],'0.3.0')
        self.assertEqual(len(files),6)
    def test_contamination_and_traversal_rejected(self):
        for name in ['data/auth.json','runtime/codex.exe','licenses/../escape','RUSTDESK-AGENT.EXE','licenses/a:stream']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                release.verify_package(self.package(extra=name))
    def test_tamper_and_missing_license_rejected(self):
        with self.assertRaises(ValueError): release.verify_package(self.package(changed=True))
        with self.assertRaises(ValueError): release.verify_package(self.package(omit='licenses/components.json'))
    def test_duplicate_entry_rejected(self):
        self.package()
        with zipfile.ZipFile(self.archive,'a') as archive:
            import warnings
            with warnings.catch_warnings():
                warnings.simplefilter('ignore',UserWarning)
                archive.writestr(release.PREFIX+'RustDesk-Agent.exe',b'synthetic')
        with self.assertRaises(ValueError): release.verify_package(self.archive)
    def test_source_inventory_and_bytes(self):
        source=self.folder/'config.rs';source.write_bytes(b'patched-source')
        name='MegaRustDesk-source/libs/hbb_common/src/config.rs'
        with patch.object(release,'source_files',return_value={name:source}):
            with zipfile.ZipFile(self.archive,'w') as archive: archive.writestr(name,b'patched-source')
            release.verify_source(self.archive)
            source.write_bytes(b'new-source')
            with self.assertRaises(ValueError): release.verify_source(self.archive)
            with zipfile.ZipFile(self.archive,'w') as archive: archive.writestr('wrong',b'new-source')
            with self.assertRaises(ValueError): release.verify_source(self.archive)
    def test_windows_reserved_paths(self):
        for name in ['licenses/CON.txt','licenses/aux','licenses/foo.','licenses/foo ','licenses/a.dll','licenses/x\\y']:
            with self.subTest(name=name),self.assertRaises(ValueError): release.safe_name(name)

if __name__=='__main__': unittest.main()

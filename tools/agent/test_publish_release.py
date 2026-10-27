"""Publisher channel regression. No real GitHub calls or signing keys."""
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch
import publish_release as publisher


class PublisherChannelTests(unittest.TestCase):
    def publish(self, with_repair=False, **kwargs):
        with tempfile.TemporaryDirectory(prefix='megarustdesk-publisher-test-') as directory:
            root = Path(directory)
            (root / 'release').mkdir()
            asset = root / 'payload.zip'
            asset.write_bytes(b'synthetic public payload')
            notes = root / 'notes.md'
            notes.write_text('Synthetic release', encoding='utf-8')
            repair=root/'tools/agent/repair-update.cmd'
            if with_repair:
                repair.parent.mkdir(parents=True);repair.write_text('synthetic repair source',encoding='ascii')
            payload = {'repository': 'URBoRTEX/MegaRustDesk', 'tag': 'agent-v9.0.0',
                       'version': '9.0.0', 'sourceCommit': 'fixture-commit'}
            calls = []

            def gh(*args):
                calls.append(tuple(map(str, args)))
                if args[:2] == ('api', 'repos/URBoRTEX/MegaRustDesk'):
                    return json.dumps({'full_name': payload['repository'], 'private': False,
                                       'fork': True, 'parent': {'full_name': 'rustdesk/rustdesk'}})
                if args[0] == 'api':
                    return json.dumps({'object': {'type': 'commit', 'sha': payload['sourceCommit']}})
                if args[:2] == ('release', 'list'):
                    return '[]'
                if args[:2] == ('release', 'view'):
                    return json.dumps({'assets': [], 'isDraft': True})
                if args[:2] == ('release', 'download'):
                    destination = Path(args[args.index('--dir') + 1])
                    shutil.copy2(asset, destination / asset.name)
                    shutil.copy2(root / 'release/SHA256SUMS.txt', destination / 'SHA256SUMS.txt')
                    if with_repair: shutil.copy2(repair,destination/repair.name)
                return ''

            with patch.object(publisher, 'ROOT', root), \
                 patch.object(publisher, 'verify_prepared', side_effect=lambda: (payload, [asset])), \
                 patch.object(publisher, 'gh', side_effect=gh):
                publisher.publish(notes, True, **kwargs)
            return calls

    def test_default_publish_is_discoverable_by_existing_updaters(self):
        calls = self.publish()
        create = next(c for c in calls if c[:2] == ('release', 'create'))
        edit = next(c for c in calls if c[:2] == ('release', 'edit'))
        self.assertIn('--prerelease=false', create)
        self.assertIn('--prerelease=false', edit)
        self.assertIn('--latest', edit)
        self.assertNotIn('preview', create[create.index('--title') + 1])

    def test_preview_is_explicit_and_not_latest(self):
        calls = self.publish(preview=True)
        create = next(c for c in calls if c[:2] == ('release', 'create'))
        edit = next(c for c in calls if c[:2] == ('release', 'edit'))
        self.assertIn('--prerelease=true', create)
        self.assertIn('--prerelease=true', edit)
        self.assertNotIn('--latest', edit)
        self.assertIn('preview', create[create.index('--title') + 1])

    def test_repair_script_is_uploaded_and_download_verified(self):
        calls=self.publish(with_repair=True)
        uploads=[c for c in calls if c[:2]==('release','upload')]
        self.assertEqual(len(uploads),3)
        self.assertTrue(any(c[3].endswith('repair-update.cmd') for c in uploads))


if __name__ == '__main__':
    unittest.main()

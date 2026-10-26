"""Regression for portable UI built/run under Windows code pages other than UTF-8."""
import builtins
import ast
import re
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
class EncodingTests(unittest.TestCase):
    def test_every_inline_document_declares_utf8_before_scripts(self):
        for name in ['index','remote','chatbox','install','cm']:
            text=(ROOT/f'src/ui/{name}.html').read_text(encoding='utf-8')
            head=text.split('<head>',1)[1].lstrip()
            with self.subTest(document=name):
                self.assertRegex(head,r'^<meta\s+[^>]*charset\s*=\s*[\"\x27]?utf-8[\"\x27]?')
    def test_generator_ignores_cp1251_default(self):
        original=builtins.open
        with tempfile.TemporaryDirectory(prefix='megarustdesk-encoding-') as folder:
            output=Path(folder)/'inline.rs'
            def legacy_open(file,mode='r',*args,**kwargs):
                path=str(file).replace('\\','/')
                if path=='src/ui/inline.rs': file=output
                elif not Path(file).is_absolute(): file=ROOT/file
                if 'b' not in mode and not args and 'encoding' not in kwargs: kwargs['encoding']='cp1251'
                return original(file,mode,*args,**kwargs)
            with patch('builtins.open',legacy_open): runpy.run_path(str(ROOT/'res/inline-sciter.py'))
            line=next(l for l in output.read_text(encoding='ascii').splitlines() if l.startswith('const _INDEX:'))
            document=ast.literal_eval(line.split(' = ',1)[1][:-1]).decode('utf-8')
            self.assertTrue('Разрешить доступ к этому компьютеру' in document,'Heading decoded with the wrong code page')
            self.assertTrue('Проверить обновления' in document,'Updater label decoded with the wrong code page')

if __name__=='__main__':unittest.main()

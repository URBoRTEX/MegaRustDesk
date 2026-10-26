"""vcpkg asset transport with verified HTTPS and mandatory expected SHA512."""
import hashlib
import pathlib
import sys
import urllib.request

url, expected, destination = sys.argv[1:]
if not url.startswith('https://'):
    raise SystemExit('Only verified HTTPS build inputs are supported')
urls = [url]
if url.startswith('https://mirror.msys2.org/'):
    urls.insert(0, url.replace('https://mirror.msys2.org/', 'https://repo.msys2.org/'))
last_error = None
for candidate in urls:
    try:
        data = urllib.request.urlopen(candidate, timeout=180).read()
        break
    except Exception as exc:
        last_error = exc
else:
    raise SystemExit(str(last_error))
if hashlib.sha512(data).hexdigest().lower() != expected.lower():
    raise SystemExit('Asset SHA512 mismatch')
pathlib.Path(destination).write_bytes(data)

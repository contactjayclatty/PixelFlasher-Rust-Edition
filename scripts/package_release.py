"""Create a clean, versioned Windows release; leave historical packages untouched."""
from pathlib import Path
import hashlib
import json
import re
import shutil
import zipfile

root = Path(__file__).resolve().parents[1]
dist = root / 'dist'
version = re.search(r'^version\s*=\s*"([^"]+)"', (root/'Cargo.toml').read_text(encoding='utf-8'), re.M).group(1)
source = dist / 'PixelFlasher-Rust-Edition-source.zip'
assert source.is_file(), 'Run package_source.py first'
with zipfile.ZipFile(source) as archive:
    assert archive.testzip() is None
source_name = f'PixelFlasher-Rust-Edition-{version}-source.zip'
shutil.copy2(source, dist / source_name)
portable_name = f'PixelFlasher-Rust-Edition-{version}-windows-x64.zip'
files = ['PixelFlasher.exe', 'pixelflasher-mcp.exe', 'README.md', 'CHANGELOG.md', 'CONTRIBUTING.md', 'AGENTS.md', 'LICENSE', 'LICENSE.upstream', 'NOTICE.md', 'THIRD_PARTY_NOTICES.md', 'manifest.json', '.agents/skills/port-validation/SKILL.md']
files += [p.relative_to(dist).as_posix() for p in (dist/'docs').glob('*') if p.is_file()]
with zipfile.ZipFile(dist / portable_name, 'w', zipfile.ZIP_DEFLATED) as archive:
    for name in sorted(files):
        path = dist / name
        assert path.is_file(), f'Missing release file: {name}'
        archive.write(path, f'PixelFlasher-Rust-Edition/{name}')
names = [portable_name, source_name]
hashes = {name: hashlib.sha256((dist/name).read_bytes()).hexdigest() for name in names}
hashes['PixelFlasher.exe'] = hashlib.sha256((dist/'PixelFlasher.exe').read_bytes()).hexdigest()
hashes['pixelflasher-mcp.exe'] = hashlib.sha256((dist/'pixelflasher-mcp.exe').read_bytes()).hexdigest()
(dist/'checksums.sha256').write_text(''.join(f'{digest}  {name}\n' for name,digest in hashes.items()), encoding='utf-8')
print(json.dumps({'version': version, 'release_assets': names + ['checksums.sha256'], 'sha256': hashes}, indent=2))

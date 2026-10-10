"""Package an explicit source allowlist; excludes user data, binaries and private keys."""
from pathlib import Path
import hashlib
import json
import zipfile

root = Path(__file__).resolve().parents[1]
dist = root / 'dist'
dist.mkdir(exist_ok=True)
files = [root / p for p in ['Cargo.toml', 'Cargo.lock', '.gitignore', 'AGENTS.md', 'README.md', 'NOTICE.md', 'LICENSE', 'LICENSE.upstream', '.gitattributes', 'CHANGELOG.md', 'CONTRIBUTING.md', 'THIRD_PARTY_NOTICES.md']]
for folder in ['src', 'crates', 'tests', 'docs', 'scripts', '.agents', '.github']:
    files.extend(p for p in (root / folder).rglob('*') if p.is_file() and '__pycache__' not in p.parts)
path = dist / 'PixelFlasher-Rust-Edition-source.zip'
with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
    for file in sorted(files):
        archive.write(file, 'PixelFlasher-Rust-Edition/' + file.relative_to(root).as_posix())
print(json.dumps({'source_archive': path.name, 'files': len(files), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}))

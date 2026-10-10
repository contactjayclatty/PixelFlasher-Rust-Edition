"""Record locked dependency licenses without publishing machine-specific paths."""
from pathlib import Path
import json
import subprocess

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=root, text=True))
packages = sorted((p for p in metadata['packages'] if p.get('source')), key=lambda p: (p['name'], p['version']))
lines = ['# Third-party dependency notices', '',
         'Generated from Cargo.lock. Includes build and platform-specific dependencies;',
         'listing a dependency does not assert it is linked into every executable.', '',
         '| Crate | Version | Declared license |', '|---|---|---|']
for package in packages:
    lines.append(f"| {package['name']} | {package['version']} | {package.get('license') or 'See license text below'} |")
for package in packages:
    directory = Path(package['manifest_path']).parent
    files = [p for p in directory.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE'))]
    if package.get('license_file'):
        files.append(directory / package['license_file'])
    # Bundled font license texts can live beneath a fonts directory.
    for folder in ['fonts', 'assets']:
        if (directory / folder).is_dir():
            files.extend(p for p in (directory / folder).rglob('*') if p.is_file() and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')))
    lines.extend(['', f"## {package['name']} {package['version']}", '',
                  f"Declared license: {package.get('license') or 'See included license file'}."])
    if package.get('repository'):
        lines.append(f"Source: {package['repository']}")
    for path in sorted(set(files)):
        if path.is_file() and path.stat().st_size < 1024 * 1024:
            content = '\n'.join(line.rstrip() for line in path.read_text(encoding='utf-8', errors='replace').splitlines())
            lines.extend(['', f"### {path.relative_to(directory).as_posix()}", '', '~~~~text', content.rstrip(), '~~~~'])
(root / 'THIRD_PARTY_NOTICES.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
print(json.dumps({'locked_dependencies': len(packages), 'output': 'THIRD_PARTY_NOTICES.md'}))

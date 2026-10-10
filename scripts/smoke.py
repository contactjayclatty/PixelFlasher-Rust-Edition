"""Development-only executable smoke test; never contacts an Android device."""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import struct
import subprocess
import sys
import tempfile
import zipfile

root = Path(__file__).resolve().parents[1]
exe = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / 'target/release/pixelflasher-rust.exe'
report = {'binary': exe.name, 'sha256': hashlib.sha256(exe.read_bytes()).hexdigest(), 'checks': [], 'device_commands': 'none'}
with tempfile.TemporaryDirectory(prefix='pixelflasher-smoke-') as folder:
    fixture = Path(folder)
    env = os.environ.copy()
    env['PIXELFLASHER_DATA_DIR'] = str(fixture / 'data')
    for key in ['EFRAME_SCREENSHOT_TO', 'PIXELFLASHER_CAPTURE_PAGE']:
        env.pop(key, None)
    image = bytearray(8192)
    image[:8] = b'ANDROID!'
    struct.pack_into('<I', image, 12, 1024)
    struct.pack_into('<I', image, 20, 1580)
    struct.pack_into('<I', image, 40, 4)
    boot = fixture / 'init_boot.img'
    boot.write_bytes(image)
    def run(*args):
        result = subprocess.run([str(exe), *map(str, args)], env=env, capture_output=True, timeout=30)
        if result.returncode:
            raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
        return json.loads(result.stdout)
    info = run('--inspect', boot)
    assert info['kind'] == 'init_boot' and info['sha256'] == hashlib.sha256(image).hexdigest()
    report['checks'].append('native executable boot inspection')
    nested = fixture / 'image-komodo.zip'
    with zipfile.ZipFile(nested, 'w') as z:
        z.write(boot, 'init_boot.img')
    factory = fixture / 'komodo-fixture-factory.zip'
    with zipfile.ZipFile(factory, 'w') as z:
        z.write(nested, 'fixture/image-komodo.zip')
        z.writestr('fixture/flash-all.bat', '@ECHO OFF\nfastboot -w update image-komodo.zip\n')
    processed = run('--process', factory)
    assert processed['product'] == 'komodo' and len(processed['images']) == 1
    assert Path(processed['images'][0]).read_bytes() == image
    report['checks'].append('native executable nested factory extraction')
    rows = run('--library')
    assert len(rows) == 1 and rows[0]['info']['sha256'] == info['sha256']
    report['checks'].append('native executable persistent image library')
    legacy = fixture / 'PixelFlasher4.db'
    with sqlite3.connect(legacy) as db:
        db.execute('CREATE TABLE BOOT(file_path TEXT,boot_hash TEXT,hardware TEXT,is_patched INTEGER,magisk_version TEXT,patch_method TEXT)')
        db.execute('INSERT INTO BOOT VALUES(?,?,?,0,NULL,NULL)', (str(boot), info['sha1'], 'komodo'))
    db.close()
    before = legacy.read_bytes()
    imported = run('--import-legacy', legacy)
    assert imported['imported'] == 1 and not imported['skipped'] and legacy.read_bytes() == before
    report['checks'].append('native executable read-only legacy import')
report['status'] = 'pass'
(root / 'artifacts').mkdir(exist_ok=True)
(root / 'artifacts/executable-smoke.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report, indent=2))

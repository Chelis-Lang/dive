#!/usr/bin/env python3
"""Verify every raw primary/secondary fixture against its generated manifest."""
import hashlib
import json
from pathlib import Path

base = Path(__file__).resolve().parent
for name in ('oracle-controls.json', 'secondary-controls.json'):
    report = json.loads((base / name).read_text())
    assert report['jar_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
    assert len(report['observations']) == report['selected']
    for path, digest in report['raw_files'].items():
        assert hashlib.sha256((base / path).read_bytes()).hexdigest() == digest, path
    print(f"{name}: {len(report['raw_files'])} fixture hashes verified")

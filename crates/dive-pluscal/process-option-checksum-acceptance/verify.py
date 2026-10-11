#!/usr/bin/env python3
"""Read-only option producer source/evidence verification; CRC equality is not identity."""
import hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
r=json.loads((BASE/'delivery.json').read_text())
assert r['gate_status']=='passed' and r['family_certified'] is False
assert r['fair_header_crc_only_limits_source_identity'] is True
assert (r['selected'],r['raw_exact'],r['admitted_per_profile'],r['finite_models'])==(524,320,320,24)
for field in ['checked_sources','archived_evidence']:
 for name,digest in r[field].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name
print('Verified',len(r['checked_sources']),'sources and',len(r['archived_evidence']),'evidence files; option/checksum families remain partial')

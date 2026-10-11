#!/usr/bin/env python3
"""Read-only source-bound process CRC evidence; no full family certification."""
import hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
r=json.loads((BASE/'delivery.json').read_text())
assert r['gate_status']=='passed' and r['family_certified'] is False and r['fair_header_crc_only_limits_source_identity'] is True
assert (r['capture_rows'],r['raw_exact'],r['admitted_per_profile'],r['rust_tests'])==(224,160,152,708)
assert (r['finite_models'],r['worker_store_controls'],r['strict_valid_witnesses'],r['invalid_witnesses'])==(44,528,22,88)
for section in ['checked_sources','archived_evidence']:
 for name,digest in r[section].items(): assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name
print('Verified',len(r['checked_sources']),'sources and',len(r['archived_evidence']),'evidence files; process checksum families remain partial')

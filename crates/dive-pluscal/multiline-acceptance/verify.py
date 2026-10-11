#!/usr/bin/env python3
"""Read-only exact-source multiline delivery verification, without full parity credit."""
import hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
receipt=json.loads((BASE/'delivery.json').read_text());assert receipt['gate_status']=='passed' and receipt['family_certified'] is False and receipt['fair_header_crc_only_limits_source_identity'] is True
for section in ['checked_sources','archived_evidence']:
 for name,expected in receipt[section].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==expected,name
assert receipt['capture_rows']==208 and receipt['sany_cst_validator_checks']==520 and receipt['strict_valid_witnesses']==14 and receipt['invalid_witnesses']==56
print('Verified source-bound multiline delivery: 208 rows; no full PlusCal, integrity or model certificate')

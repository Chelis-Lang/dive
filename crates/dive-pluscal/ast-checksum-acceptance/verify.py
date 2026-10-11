#!/usr/bin/env python3
"""Read-only source-bound AST/Full checksum delivery verification."""
import hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
receipt=json.loads((BASE/'delivery.json').read_text());assert receipt['gate_status']=='passed' and receipt['family_certified'] is False and receipt['reference_crc_derivation_verified_in_supported_scope'] is True and receipt['fair_header_crc_only_limits_source_identity'] is True
for section in ['checked_sources','archived_evidence']:
 for name,expected in receipt[section].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==expected,name
print('Verified source-bound bounded AST/Full delivery:',receipt['capture_rows'],'rows; no full family, checksum-integrity or model certificate')

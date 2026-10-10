#!/usr/bin/env python3
"""Read-only delivery/source and every original/final raw evidence checksum."""
import hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def main():
 receipt=json.loads((BASE/'delivery.json').read_text());assert receipt['gate_status']=='passed' and receipt['family_certified'] is False and receipt['algorithm_checksum_verified'] is False
 for section in ['checked_sources','archived_evidence']:
  for name,expected in receipt[section].items():
   actual=hashlib.sha256((ROOT/name).read_bytes()).hexdigest();assert actual==expected, name
 print('Verified complete source-bound checksum/editor delivery:',receipt['capture_rows'],'primary rows;',receipt['native_artifact_checks'],'native artifact checks; no full AST checksum claim')
if __name__=='__main__':main()

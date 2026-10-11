#!/usr/bin/env python3
"""Read back bounded producer inputs/evidence; no clean-source or family retagging."""
from pathlib import Path
import hashlib,json
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
r=json.loads((BASE/'delivery.json').read_text())
assert r['source_commit']is None and r['source_status']=='uncommitted-frozen-during-gates-and-consumers'
assert r['gate_status']=='passed'and r['family_certified']is False and r['fair_header_crc_only_limits_source_identity']is True
assert (r['selected'],r['workspace_tests'],r['finite_models'],r['worker_store_profile_controls'],r['strict_valid_witnesses'],r['rejected_witness_mutants'])==(752,719,24,288,12,48)
assert r['public_summary']=={'admitted-exact-cst-sany-validator':480,'invalid-primary-uncredited':16,'rejection-accounted':32,'named-public-refusal':224}
for field in ['checked_sources','unchanged_build_inputs','archived_evidence']:
 for name,digest in r[field].items():assert sha(ROOT/name)==digest,name
for name,digest in r['checked_sources'].items():assert sha(BASE/'uncommitted-producer-source'/name)==digest,name
for folder,executed in [('stage-separated','compare-stage-separated.py.txt'),('public-module-names','public-module-names.py.txt'),('finite-models','models.py.txt'),('guides','guides.py.txt'),('checksum-corruption-controls','checksum-controls.py.txt'),('comparison-negative-controls','report-controls.py.txt')]:
 report=json.loads((BASE/folder/'report.json').read_text());assert sha(BASE/'executed-runners'/executed)==report['script_sha256'],folder
print('Verified',len(r['checked_sources']),'original production inputs and',len(r['archived_evidence']),'archived evidence files; both families remain partial')

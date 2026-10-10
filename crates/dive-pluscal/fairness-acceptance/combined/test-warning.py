#!/usr/bin/env python3
"""Reject a full native artifact forged with a sequential-only warning."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('combined_compare',HERE/'compare.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--native',type=Path,required=True)
args=parser.parse_args()
manifest=json.loads((HERE/'fixtures/manifest.json').read_text())
row=next(row for row in manifest['rows'] if row['id']=='FairCombined_P_set_weak_option_nof')
source=(HERE/'cases'/row['input']).read_bytes()
document=module.execute(args.native,source,row['options'])
assert module.outcome(row,document,source,HERE/'fixtures',args.native)['outcome']=='token-artifact-match'
forged=copy.deepcopy(document)
forged['artifact']['warnings'].append({'code':'PCALW_FAIR_OPTION','span':{'start':0,'end':len(source)},'message':"Option `nof' specified for --fair algorithm."})
outcome=module.outcome(row,forged,source,HERE/'fixtures',args.native)
assert outcome['outcome']=='disagree',outcome
report={'schema_version':1,'control':'invented-sequential-warning','outcome':outcome,
        'native_sha256':module.sha(args.native.read_bytes()),
        'manifest_sha256':module.sha((HERE/'fixtures/manifest.json').read_bytes()),
        'comparator_sha256':module.sha((HERE/'compare.py').read_bytes()),
        'family_certified':False}
(HERE/'warning-control.json').write_text(json.dumps(report,indent=2)+'\n')
print('Rejected invented sequential-only warning')

#!/usr/bin/env python3
"""Verify graph evidence; incomplete strict replay earns no violation credit."""
import argparse
from collections import Counter
import gzip, hashlib, json, re
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def content(capture,directory):
    for stream in ('stdout','stderr'):
        if stream+'_sha256' in capture:
            data=gzip.decompress((directory/capture[stream]).read_bytes())
            assert hashlib.sha256(data).hexdigest()==capture[stream+'_sha256']
    return json.loads(gzip.decompress((directory/capture['stdout']).read_bytes()))
def raw(obj,directory):
    if isinstance(obj,dict):
        for stream in ('stdout','stderr'):
            if stream+'_sha256' in obj:
                data=gzip.decompress((directory/obj[stream]).read_bytes())
                assert hashlib.sha256(data).hexdigest()==obj[stream+'_sha256']
        if 'path' in obj and 'sha256' in obj:
            path=directory/obj['path'];data=path.read_bytes()
            if path.suffix=='.gz':data=gzip.decompress(data)
            assert hashlib.sha256(data).hexdigest()==obj['sha256']
        for value in obj.values():raw(value,directory)
    elif isinstance(obj,list):
        for value in obj:raw(value,directory)
def main(args):
    receipt=json.loads((BASE/'delivery.json').read_text())
    assert receipt['gate_status']=='passed' and len(receipt['gate_logs'])==3
    assert receipt['family_certified'] is False and receipt['tlc_violation_certified'] is False
    for key in ['reports','checked_sources','gate_logs','archived_evidence']:
        for name,expected in receipt[key].items():assert sha(ROOT/name)==expected,name
    assert receipt['native_sha256']==sha(args.native) and receipt['example_sha256']==sha(args.example)
    manifest=json.loads((BASE/'fixtures/manifest.json').read_text());comparison=json.loads((BASE/'comparison.json').read_text())
    assert len(manifest['rows'])==comparison['selected']==len(comparison['rows'])==120
    assert {r['id'] for r in manifest['rows']}=={r['id'] for r in comparison['rows']}
    assert comparison['model_proved'] is False
    assert comparison['manifest_sha256']==sha(BASE/'fixtures/manifest.json') and comparison['comparator_sha256']==sha(BASE/'compare.py') and comparison['native_sha256']==sha(args.example)
    assert comparison['summary']==dict(Counter(r['outcome'] for r in comparison['rows']))=={'token-artifact-match':68,'rejection-match':19,'native-refusal':29,'invalid-reference-artifact':4}
    invalid=0
    for row in manifest['rows']:
        assert sha(BASE/'cases'/row['input'])==row['input_sha256'];raw(row,BASE/'fixtures')
        invalid+=row.get('sany',{}).get('status')=='rejected'
    assert invalid==8
    report=json.loads((BASE/'validation/report.json').read_text())
    assert report['native_sha256']==sha(args.native) and report['cst_dumper_sha256']==sha(args.example)
    assert report['script_sha256']==sha(BASE/'validate.py') and report['selection_report_sha256']==sha(BASE/'comparison.json')
    assert report['family_certified'] is False and report['selected_artifacts']==len(report['artifact_rows'])==68
    assert report['artifact_summary']=={'native-sany-match':68}
    for row in report['artifact_rows']:
        assert row['operator_structure_equal'] is True and sha(BASE/'validation'/row['artifact'])==row['artifact_sha256']
    assert len(report['model_rows'])==130 and report['model_summary']=={'finite-model-match':64,'disagree':66}
    for row in report['model_rows']:
        assert sha(BASE/'validation'/row['source'])==row['source_sha256'] and sha(BASE/'validation'/row['config'])==row['config_sha256']
        assert len(row['workers_stores'])==6 and {(c['workers'],c['store']) for c in row['workers_stores']}=={(w,s) for w in [1,4,8] for s in ['fp','exact']}
        assert all(c['outcome']=='match' for c in row['workers_stores'])
        doc=content(row['native'],BASE/'validation')
        assert doc['options']['profile']=='tlc' and doc['verdict']==row['expected_verdict']
        assert row['primary']['exit_code']==row['native']['exit_code']==doc['tlc']['exit_status']=={'success':0,'invariant-violation':12,'deadlock':11}[row['expected_verdict']]
        assert all(doc['statistics'].get(k)==v for k,v in row['primary_counts'].items() if v is not None)
        assert (row['expected_verdict']=='success') is (row['outcome']=='finite-model-match')
    raw(report,BASE/'validation')
    qualified=json.loads((BASE/'qualified-replay/report.json').read_text())
    assert qualified['original_report_sha256']==sha(BASE/'validation/report.json') and qualified['native_sha256']==sha(args.native) and qualified['script_sha256']==sha(BASE/'qualified-replay.py')
    assert qualified['family_certified'] is False and qualified['tlc_violation_certified'] is False
    assert qualified['summary']=={'strict-complete-model-match':64,'strict-witness-pending':62,'strict-deadlock-pending':4}
    assert {r['id'] for r in qualified['rows']}=={r['id'] for r in report['model_rows']}
    assert len(qualified['controls'])==330 and Counter(c['result'] for c in qualified['controls'])=={'valid':62,'invalid':264,'unsupported':4}
    for row in qualified['rows']:
        if row['outcome']!='strict-complete-model-match':
            assert row['tlc_certified'] is False and row['strict_validation']['result']=='unsupported'
            assert row['strict_validation']['unsupported']=='runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)'
    for control in qualified['controls']:
        assert control['strict_tlc_credit'] is False and control['profile']=='dive-default'
        assert control['exit_code']=={'valid':0,'invalid':1,'unsupported':3}[control['result']]
    raw(qualified,BASE/'qualified-replay')
    mutations=json.loads((BASE/'mutation-controls.json').read_text());assert mutations['control_count']==len(mutations['rows'])==39 and all(r['outcome']['outcome']=='disagree' for r in mutations['rows'])
    for directory,script,count in [('admission','check-admission.py',16),('guide-api','check-guide-api.py',2),('guide-cli','check-guide-cli.py',4)]:
        doc=json.loads((BASE/directory/'report.json').read_text());assert doc['script_sha256']==sha(BASE/script) and doc['controls']==len(doc['rows'])==count
        if directory=='admission':assert all(r['passed'] for r in doc['rows'])
        else:
            assert doc['guide_sha256']==sha(ROOT/'docs/pluscal-procedure-graphs.md')
            assert all(r['result']=='artifact' and r['model_proved'] is False and r['exhaustive'] is False for r in doc['rows'])
        raw(doc,BASE/directory)
    failure=json.loads((BASE/'initial-validator-selection/failure.json').read_text())
    raw(failure,BASE/'initial-validator-selection')
    assert failure['failed_script_sha256']==sha(BASE/'initial-validator-selection/validate.py')
    failed_raw=gzip.decompress((BASE/'initial-validator-selection'/failure['primary']['stdout']).read_bytes())
    failed_stats=re.findall(rb'([0-9]+) states generated, ([0-9]+) distinct states found, ([0-9]+) states left on queue',failed_raw)
    assert failed_stats==[(b'4',b'4',b'0')] and b'Error' in failed_raw
    assert failure['expected_exit']==0 and failure['observed_primary_exit']==11 and failure['model_proved'] is False
    print(json.dumps({'selection':comparison['summary'],'sany_cst_artifacts':68,'strict_complete_models':64,'strict_invariant_pending':62,'strict_deadlock_pending':4,'width_store_controls':780,'qualified_default_valid':62,'qualified_default_invalid':264,'qualified_default_deadlock_pending':4,'primary_invalid_uncredited':8,'family_certified':False,'tlc_violation_certified':False}))
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);p.add_argument('--example',type=Path,required=True);main(p.parse_args())

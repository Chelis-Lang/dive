#!/usr/bin/env python3
"""Check every bound byte and complete disposition before accepting a delivery."""
import argparse,collections,gzip,hashlib,importlib.util,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text())
def main(args):
    receipt=read(BASE/'delivery.json')
    assert receipt['native_sha256']==sha(args.native) and receipt['example_sha256']==sha(args.example) and receipt['harness_sha256']==sha(args.harness)
    assert receipt['family_certified'] is False and receipt['tlc_violation_certified'] is False and receipt['gate_status']=='passed'
    for category in ['reports','checked_sources','gate_logs','archived_evidence']:
        for file,digest in receipt[category].items():assert sha(ROOT/file)==digest,(category,file)
    selected=artifacts=invalid=complete=pending=deadlocks=models=valid=mutants=0
    for directory in [BASE]:
        manifest=read(directory/'primary/manifest.json');report=read(directory/'comparison.json');selection=read(directory/'selection.json')
        assert report['native_sha256']==sha(args.example) and report['comparator_sha256']==sha(directory/'compare.py') and report['manifest_sha256']==sha(directory/'primary/manifest.json') and report['model_proved'] is False
        assert report['selected']==len(manifest['rows'])==selection['selected']
        assert set(r['id'] for r in manifest['rows'])==set(r['id'] for r in selection['rows'])==set(r['id'] for r in report['rows'])
        assert report['summary']==dict(collections.Counter(r['outcome'] for r in report['rows'])) and not report['summary'].get('disagree')
        spec=importlib.util.spec_from_file_location('owned_compare',directory/'compare.py');comparison=importlib.util.module_from_spec(spec);spec.loader.exec_module(comparison)
        results={r['id']:r for r in report['rows']}
        for row in manifest['rows']:
            source=(directory/'cases'/row['input']).read_bytes();assert hashlib.sha256(source).hexdigest()==row['input_sha256']
            comparison.verify(directory/'primary',row['primary'])
            if 'sany' in row:comparison.verify(directory/'primary',row['sany'])
            for field in ['translated','config']:
                if field in row:assert sha(directory/'primary'/row[field]['path'])==row[field]['sha256']
            document=comparison.execute(args.example,source,row['options']);fresh=comparison.outcome(row,document,source,directory/'primary',args.example)
            assert fresh['outcome']==results[row['id']]['outcome'],row['id']
            if row.get('sany',{}).get('status')=='rejected':
                invalid+=1;assert fresh['outcome']!='token-artifact-match'
        selected+=len(manifest['rows']);artifacts+=report['summary'].get('token-artifact-match',0)
        if (directory/'validation/report.json').exists():
            validation=read(directory/'validation/report.json');qualified=read(directory/'qualified-replay/report.json')
            assert validation['native_sha256']==sha(args.native) and validation['cst_dumper_sha256']==sha(args.example) and validation['script_sha256']==sha(directory/'validate.py')
            assert validation['artifact_summary']=={'native-sany-match':validation['selected_artifacts']}
            assert qualified['native_sha256']==sha(args.native) and qualified['original_report_sha256']==sha(directory/'validation/report.json') and qualified['script_sha256']==sha(directory/'qualified-replay.py') and qualified['family_certified'] is False and qualified['tlc_violation_certified'] is False
            assert qualified['summary']==dict(collections.Counter(r['outcome'] for r in qualified['rows']))
            complete+=qualified['summary'].get('strict-complete-model-match',0);pending+=qualified['summary'].get('strict-witness-pending',0);deadlocks+=qualified['summary'].get('strict-deadlock-pending',0)
            for row in validation['model_rows']:
                assert len(row['workers_stores'])==6 and all(c['outcome']=='match' for c in row['workers_stores']);models+=1
                assert sha(directory/'validation'/row['source'])==row['source_sha256'] and sha(directory/'validation'/row['config'])==row['config_sha256']
            for control in qualified['controls']:
                assert control['strict_tlc_credit'] is False and control['profile']=='dive-default'
                if control['control']=='original':assert control['result']=='valid' and control['exit_code']==0;valid+=1
                else:assert control['result']=='invalid' and control['exit_code']==1;mutants+=1
    assert (selected,artifacts,invalid,complete,pending,deadlocks,models,valid,mutants)==(166,94,11,74,66,2,142,68,272)
    for name,count in [('admission',22),('api-admission',22),('guide-cli',4),('guide-api',2)]:
        report=read(BASE/name/'report.json');assert report['controls']==count
        if 'guide_sha256' in report:assert report['guide_sha256']==sha(ROOT/'docs/pluscal-multiple-procedures.md')
    options=read(BASE/'option-refusals/report.json');assert options['controls']==4
    assert options['native_sha256']==sha(args.native)
    assert read(BASE/'mutation-controls.json')['control_count']==39
    budget=read(BASE/'budgets/report.json');assert len(budget['primary_rows'])==2 and len(budget['native_controls'])==32
    assert all(c['result']['model_proved'] is False and c['result']['exhaustive'] is False and c['result']['result'] in ['unsupported','resource-limit'] for c in budget['native_controls'])
    print('Verified complete 166-source / 94-artifact / 142-model delivery and all bound bytes')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['native','example','harness']:p.add_argument('--'+key,type=Path,required=True)
    main(p.parse_args())

#!/usr/bin/env python3
"""Account for strict namespace refusals separately from default-profile replay.

This does not change the failed strict campaign or certify TLC-profile witnesses.
"""
import argparse
from collections import Counter
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw(directory, capture):
    data=gzip.decompress((directory/capture['stdout']).read_bytes())
    assert hashlib.sha256(data).hexdigest()==capture['stdout_sha256']
    return json.loads(data)


def main(args):
    base=Path(__file__).resolve().parent;directory=base/'validation'
    report=json.loads((directory/'report.json').read_text())
    assert report['native_sha256']==sha(args.native)
    assert report['artifact_summary']=={'native-sany-match':68}
    args.output.mkdir(exist_ok=True)
    rows=[];controls=[]
    for row in report['model_rows']:
        document=raw(directory,row['native'])
        assert row['primary']['exit_code']==row['native']['exit_code']
        assert all(document['statistics'].get(k)==v for k,v in row['primary_counts'].items() if v is not None)
        assert len(row['workers_stores'])==6 and all(c['outcome']=='match' for c in row['workers_stores'])
        if row['outcome']=='finite-model-match':
            assert not row['independent_validation'] and document['verdict']=='success' and row['primary']['exit_code']==0
            rows.append({'id':row['id'],'outcome':'strict-complete-model-match'})
            continue
        assert row['outcome']=='disagree' and row['id'].endswith(('.violation','.deadlock'))
        assert document['verdict']==row['expected_verdict']
        assert row['expected_verdict'] in ('invariant-violation','deadlock')
        assert all(c['passed'] for c in row['independent_validation'] if c['control']!='original')
        original=next(c for c in row['independent_validation'] if c['control']=='original')
        observed=raw(directory,original['capture'])
        assert observed['result']=='unsupported' and original['capture']['exit_code']==3
        assert observed['unsupported']=='runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)'
        trace=document['counterexample']['trace'];name=row['id'].split('.')[0]
        rows.append({'id':row['id'],'outcome':'strict-deadlock-pending' if row['expected_verdict']=='deadlock' else 'strict-witness-pending','strict_validation':observed,'tlc_certified':False})
        with tempfile.TemporaryDirectory(prefix='dive-procedure-qualified-') as temporary:
            work=Path(temporary);(work/(name+'.tla')).write_bytes((directory/row['source']).read_bytes());(work/(name+'.cfg')).write_bytes((directory/row['config']).read_bytes())
            for kind in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']:
                candidate=json.loads(json.dumps(trace))
                if kind=='invalid-init':candidate['states'][0]['values'].pop('x',None)
                elif kind=='invalid-next':candidate['states'][-1]['values']['pc']={}
                elif kind=='missing-final-variable':candidate['states'][-1]['values'].pop('x',None)
                elif kind=='invalid-closing':candidate['states'].append(json.loads(json.dumps(candidate['states'][0])))
                (work/'trace.json').write_text(json.dumps(candidate))
                command=[str(args.native),'validate-trace',name+'.tla','--config',name+'.cfg','--trace','trace.json','--seed','0','--fp','0','--output','json']
                environment=os.environ.copy();environment.pop('DIVE_COMPAT',None)
                process=subprocess.run(command,cwd=work,capture_output=True,timeout=20,env=environment);result=json.loads(process.stdout)
                expected='unsupported' if kind=='original' and row['expected_verdict']=='deadlock' else 'valid' if kind=='original' else 'invalid';exit_code=3 if expected=='unsupported' else 0 if kind=='original' else 1
                assert result['result']==expected and process.returncode==exit_code,(row['id'],kind,result)
                if expected=='unsupported':assert result['unsupported'].startswith('deadlock: the independent finite-cover check could not decide absence of successors;')
                captures={}
                for stream in ['stdout','stderr']:
                    content=getattr(process,stream);filename=row['id']+'.'+kind+'.'+stream+'.gz';(args.output/filename).write_bytes(gzip.compress(content,mtime=0));captures[stream]={'path':filename,'sha256':hashlib.sha256(content).hexdigest()}
                controls.append({'id':row['id'],'control':kind,'profile':'dive-default','result':result['result'],'exit_code':process.returncode,'strict_tlc_credit':False,'capture':captures})
    summary=dict(Counter(row['outcome'] for row in rows));assert summary=={'strict-complete-model-match':64,'strict-witness-pending':62,'strict-deadlock-pending':4},summary
    assert len(controls)==330
    result={'schema_version':1,'script_sha256':sha(Path(__file__)),'original_report_sha256':sha(directory/'report.json'),'native_sha256':sha(args.native),'family_certified':False,'tlc_violation_certified':False,'scope':'Default/canonical replay is qualified separate evidence; no #124 guard relaxation or strict validation credit. Original strict disagreement campaign remains unchanged.','summary':summary,'rows':rows,'controls':controls}
    (args.output/'report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(summary))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--native',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);main(parser.parse_args())

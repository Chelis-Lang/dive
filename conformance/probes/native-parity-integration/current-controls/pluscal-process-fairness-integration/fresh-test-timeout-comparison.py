#!/usr/bin/env python3
"""An incomplete reference decision cannot excuse a forged native proof or schema."""
import argparse,copy,importlib.util,json
from pathlib import Path
BASE=Path('/Users/jeff/code/dive-pluscal-process-shipping-work/crates/dive-pluscal/procedure-fairness-acceptance')

def main(native):
    controls=[]
    for directory in [BASE,BASE/'corrected-modifiers']:
        spec=importlib.util.spec_from_file_location('timeout_compare',directory/'compare.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        manifest=json.loads((directory/'primary/manifest.json').read_text())
        for row in manifest['rows']:
            if row['primary']['status']!='timeout' and row.get('sany',{}).get('status')!='timeout':continue
            source=(directory/'cases'/row['input']).read_bytes();actual=module.execute(native,source,row['options'])
            assert module.outcome(row,actual,source,directory/'primary',native)['outcome']=='oracle-error'
            for mutation in ['false-proof','false-exhaustive','missing-proof','internal-error','bad-exit','bad-disposition','wrong-stage','wrong-tokens','missing-map','missing-actions']:
                doc=copy.deepcopy(actual)
                if mutation=='false-proof':doc['model_proved']=True
                elif mutation=='false-exhaustive':doc['exhaustive']=True
                elif mutation=='missing-proof':doc.pop('model_proved')
                elif mutation=='internal-error':doc={'stage':'Generation','model_proved':False,'exhaustive':False,'code':'DIVE-P0050','error':{'message':'bug'}}
                elif mutation=='bad-exit':doc['exit_code']=255
                elif mutation=='bad-disposition':doc['accepted']=False
                elif mutation=='wrong-stage':doc['stage']='unknown'
                elif mutation=='wrong-tokens':doc['tokens'].append('TRUE')
                elif mutation=='missing-map':doc['artifact']['mappings']=[]
                elif mutation=='missing-actions':doc['artifact']['actions']=[]
                outcome=module.outcome(row,doc,source,directory/'primary',native)
                assert outcome['outcome']=='disagree',(row['id'],mutation,outcome)
                controls.append({'id':row['id'],'mutation':mutation,'outcome':outcome})
    assert len(controls)==60
    report={'schema_version':1,'script_sha256':module.sha(Path(__file__).read_bytes()),'native_sha256':module.sha(native.read_bytes()),'comparator_sha256':module.sha((BASE/'compare.py').read_bytes()),'controls':len(controls),'family_certified':False,'rows':controls}
    (Path('/Users/jeff/code/dive-pluscal-process-shipping-work/conformance/probes/native-parity-integration/current-controls/pluscal-process-fairness-integration/timeout-mutation-controls.json')).write_text(json.dumps(report,indent=2)+'\n');print('Rejected all60 timeout-row full-document corruptions')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);main(p.parse_args().native)

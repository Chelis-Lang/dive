#!/usr/bin/env python3
"""Every selected original row remains intact; forged full documents fail."""
import argparse,copy,importlib.util,json
from pathlib import Path
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('macro_compare',HERE/'compare.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
def main(native):
    manifest=json.loads((HERE/'fixtures/manifest.json').read_text());controls=[]
    for name in ('Recursive_P_self_base','Recursive_P_self_tail_local','Recursive_P_unused_invalid'):
        row=next(row for row in manifest['rows'] if row['id']==name);source=(HERE/'cases'/row['input']).read_bytes();actual=module.execute(native,source,row['options']);baseline=module.outcome(row,actual,source,HERE/'fixtures',native)
        assert baseline['outcome'] in ('token-artifact-match','invalid-reference-artifact')
        mutations=['false-proof','false-exhaustive','missing-proof','internal-error','bad-disposition','bad-exit','wrong-stage','wrong-tokens','wrong-body','missing-map','invalid-map','missing-actions','wrong-pc']
        if name.endswith('auto_label'):mutations+=['missing-labels','wrong-position','wrong-definition','outside-definition','wrong-origin']
        for mutation in mutations:
            doc=copy.deepcopy(actual)
            if mutation=='false-proof':doc['model_proved']=True
            elif mutation=='false-exhaustive':doc['exhaustive']=True
            elif mutation=='missing-proof':doc.pop('model_proved')
            elif mutation=='internal-error':doc={'stage':'Generation','model_proved':False,'exhaustive':False,'code':'DIVE-P0050','error':{'message':'bug'}}
            elif mutation=='bad-disposition':doc['accepted']=False
            elif mutation=='bad-exit':doc['exit_code']=255
            elif mutation=='wrong-stage':doc['stage']='internal-error'
            elif mutation=='wrong-tokens':doc['tokens'].append('TRUE')
            elif mutation=='wrong-body':doc['artifact']['tla']+='Bogus == TRUE\n'
            elif mutation=='missing-map':doc['artifact']['mappings']=[]
            elif mutation=='invalid-map':doc['artifact']['mappings'][0]['generated']['end']=999999
            elif mutation=='missing-actions':doc['artifact']['actions']=[]
            elif mutation=='wrong-pc':doc['artifact']['program_counter']=False
            elif mutation=='missing-labels':doc['artifact']['inserted_labels']=[]
            elif mutation=='wrong-position':doc['artifact']['inserted_labels'][0]['position']['column']+=1
            elif mutation=='wrong-definition':doc['artifact']['inserted_labels'][0]['macro_definition_position']['column']+=1
            elif mutation=='outside-definition':doc['artifact']['inserted_labels'][0]['macro_definition']['end']=999999
            elif mutation=='wrong-origin':doc['artifact']['inserted_labels'][0]['macro_definition']={'start':0,'end':1}
            outcome=module.outcome(row,doc,source,HERE/'fixtures',native)
            if outcome['outcome']!='disagree':raise ValueError('masked '+name+':'+mutation)
            controls.append({'id':name,'mutation':mutation,'outcome':outcome})
    report={'schema_version':1,'comparator_sha256':module.sha((HERE/'compare.py').read_bytes()),'manifest_sha256':module.sha((HERE/'fixtures/manifest.json').read_bytes()),'native_sha256':module.sha(native.read_bytes()),'control_count':len(controls),'family_certified':False,'rows':controls}
    (HERE/'mutation-controls.json').write_text(json.dumps(report,indent=2)+'\n');print('Rejected',len(controls),'full-document corruptions')
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--native',type=Path,required=True);main(parser.parse_args().native)

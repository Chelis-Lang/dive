#!/usr/bin/env python3
"""Full-document corruption controls; no primary rows are changed."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('translation_compare',HERE/'compare.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

def main(native):
    manifest=json.loads((HERE/'fixtures/manifest.json').read_text())
    row=next(row for row in manifest['rows'] if row['id']=='Multi_P_idset')
    source=(HERE/'cases'/row['input']).read_bytes()
    actual=module.execute(native,source,row['options'])
    assert module.outcome(row,actual,source,HERE/'fixtures',native)['outcome']=='token-artifact-match'
    controls=[]
    for mutation in ['false-proof','missing-exploration','false-exhaustive','wrong-stage','bad-disposition','bad-exit','removed-process','reordered-processes','second-process-name','second-process-storage','second-local','second-domain','second-source','enclosing-source','invented-label','wrong-tokens','changed-body','missing-map','wrong-pc','missing-actions','internal-error','invalid-map','wrong-configuration','wrong-process-name','wrong-storage-kind','missing-local','wrong-domain-span','wrong-valid-domain-span']:
        document=copy.deepcopy(actual)
        if mutation=='false-proof':document['model_proved']=True
        elif mutation=='missing-exploration':document.pop('exhaustive')
        elif mutation=='false-exhaustive':document['exhaustive']=True
        elif mutation=='wrong-stage':document['stage']='internal-error'
        elif mutation=='bad-disposition':document['accepted']=False
        elif mutation=='bad-exit':document['exit_code']=255
        elif mutation=='removed-process':document['artifact']['processes'].pop()
        elif mutation=='reordered-processes':document['artifact']['processes'].reverse()
        elif mutation=='second-process-name':document['artifact']['processes'][1]['name']='Bogus'
        elif mutation=='second-process-storage':document['artifact']['processes'][1]['binding_kind']='Equal'
        elif mutation=='second-local':document['artifact']['processes'][1]['local_variables']=[]
        elif mutation=='second-domain':document['artifact']['processes'][1]['id_source']={'start':0,'end':4}
        elif mutation=='second-source':document['artifact']['processes'][1]['source']={'start':0,'end':4}
        elif mutation=='enclosing-source':document['artifact']['processes'][1]['source']={'start':0,'end':len(source)}
        elif mutation=='invented-label':document['artifact']['inserted_labels']=[{'source':{'start':0,'end':4},'name':'L','position':{'line':1,'column':1}}]
        elif mutation=='wrong-tokens':document['tokens'].append('TRUE')
        elif mutation=='changed-body':document['artifact']['tla']+='Bogus == TRUE\n'
        elif mutation=='missing-map':document['artifact']['mappings']=[]
        elif mutation=='wrong-pc':document['artifact']['program_counter']=False
        elif mutation=='missing-actions':document['artifact']['actions']=[]
        elif mutation=='invalid-map':document['artifact']['mappings'][0]['generated']['end']=99999999
        elif mutation=='missing-labels':document['artifact']['inserted_labels']=[]
        elif mutation=='wrong-label-name':document['artifact']['inserted_labels'][0]['name']='Bogus'
        elif mutation=='wrong-label-position':document['artifact']['inserted_labels'][0]['position']['column']+=1
        elif mutation=='wrong-configuration':document['artifact']['configuration']='CONSTANT defaultInitValue = Fake\n'
        elif mutation=='wrong-process-name':document['artifact']['processes'][0]['name']='Bogus'
        elif mutation=='wrong-storage-kind':document['artifact']['processes'][0]['binding_kind']='Member'
        elif mutation=='missing-local':document['artifact']['processes'][0]['local_variables']=[]
        elif mutation=='wrong-domain-span':document['artifact']['processes'][0]['id_source']['end']=99999999
        elif mutation=='wrong-valid-domain-span':document['artifact']['processes'][0]['id_source']={'start':0,'end':4}
        elif mutation=='internal-error':document={'stage':'Generation','model_proved':False,'code':'DIVE-P0050','error':{'message':'bug'}}
        outcome=module.outcome(row,document,source,HERE/'fixtures',native)
        if outcome['outcome']!='disagree':raise ValueError('masked corrupted document '+mutation)
        controls.append({'control':mutation,'outcome':outcome})
    report={'schema_version':1,'control_count':len(controls),'comparator_sha256':module.sha((HERE/'compare.py').read_bytes()),'manifest_sha256':module.sha((HERE/'fixtures/manifest.json').read_bytes()),'native_sha256':module.sha(native.read_bytes()),'family_certified':False,'rows':controls}
    (HERE/'mutation-controls.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Rejected all '+str(len(controls))+' corrupted complete documents')
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--native',type=Path,required=True)
    main(parser.parse_args().native)

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
    row=next(row for row in manifest['rows'] if row['id']=='Proc_P_set_implicit_label')
    source=(HERE/'cases'/row['input']).read_bytes()
    actual=module.execute(native,source,row['options'])
    assert module.outcome(row,actual,source,HERE/'fixtures',native)['outcome']=='token-artifact-match'
    controls=[]
    for mutation in ['false-proof','wrong-tokens','changed-body','missing-map','wrong-pc','missing-actions','internal-error','invalid-map','missing-labels','wrong-label-name','wrong-label-position','wrong-configuration','wrong-process-name','wrong-storage-kind','missing-local','wrong-domain-span','wrong-valid-domain-span']:
        document=copy.deepcopy(actual)
        if mutation=='false-proof':document['model_proved']=True
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
        elif mutation=='wrong-storage-kind':document['artifact']['processes'][0]['binding_kind']='Equal'
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

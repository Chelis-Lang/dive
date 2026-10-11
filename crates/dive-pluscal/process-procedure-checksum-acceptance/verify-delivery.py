#!/usr/bin/env python3
"""Read exact selected producer inputs, raw campaigns and qualified portable evidence."""
import argparse, ast, base64, gzip, hashlib, json, re
from pathlib import Path, PurePosixPath
SOURCE = '0064beb1f1ec4edc246e3b20e017cecad9a626b3'
PREFIX = 'crates/dive-pluscal/process-procedure-checksum-acceptance/'
def need(ok, message):
    if not ok:
        raise ValueError(message)
def sha(data):
    return hashlib.sha256(data).hexdigest()
def rel(name):
    p = PurePosixPath(name)
    need(not p.is_absolute() and '..' not in p.parts, 'unsafe path')
    return p.as_posix()
def verify(base, binaries=None):
    d = json.loads((base/'delivery.json').read_bytes())
    need(d['source_commit'] == SOURCE and d['all_consumers_closed'] and not d['family_certified'] and not d['model_proved'], 'delivery qualification')
    raw = (base/rel(d['records_archive'])).read_bytes()
    need(sha(raw) == d['records_archive_sha256'], 'portable archive identity')
    records = {}
    for r in json.loads(gzip.decompress(raw)):
        n = rel(r['path']); b = base64.b64decode(r['data'], validate=True)
        need(n not in records and sha(b) == r['sha256'] == d['files_sha256'].get(n), 'portable record:'+n)
        records[n] = b
    need(set(records) == set(d['files_sha256']) and len(records) == d['records_count'] and sum(map(len,records.values())) == d['raw_bytes'], 'complete portable records')
    def doc(n): return json.loads(records[rel(n)])
    def packed(n): return gzip.decompress(records[rel(n)])
    def source(n): return packed('producer/source/'+rel(n)+'.gz')
    index = doc('producer/index.json'); p = doc('final-runtime/producer.json')
    need(index['source_commit'] == p['source_commit'] == SOURCE and index['source_tree'] == p['source_tree'] == d['source_tree'] and len(index['inputs']) == 6542 and sha(records['producer/index.json']) == p['selected_source_index_sha256'], 'source producer')
    for n,m in index['inputs'].items():
        b=source(n); need(sha(b)==m['sha256'] and len(b)==m['bytes'], 'selected source:'+n)
    literal = doc('literal-include-audit.json')
    need(literal['all_captured_before_gate'] and literal['missing']==[] and literal['unique_literal_targets']==65 and literal['literal_occurrences']==70, 'literal includes')
    for r in literal['rows']:
        need(sha(source(r['target']))==r['sha256'], 'literal target')
    external = doc('runtime-inputs-declaration.json')
    need(sha(records['runtime-inputs-declaration.json'])==p['runtime_external_inputs_sha256'] and external['declared_before_native_runtime'] and len(external['inputs'])==5060 and external['selected_main_sources']==242 and external['separate_none_settings']==56 and external['new_unique_none_sources']==0, 'external inputs')
    external_count=0
    for n,h in external['inputs'].items():
        if n in ['external-java','external-jar']:
            continue # JRE executable/jar identities recorded separately; not hermetic.
        need(sha(packed('runtime-inputs-source/'+rel(n)+'.gz'))==h, 'external archive:'+n)
        if n.startswith(('reference/','nof-reference/','markdown-guide-base/','adapter/','reference-library/')):
            need(n in records and sha(records[n])==h, 'actual external input:'+n)
        external_count+=1
    need(external_count==5058, 'external archive census')
    for n,m in p['binaries'].items():
        need(m['sha256']==d['binary_sha256'][n], 'binary declaration')
        if binaries:
            b=(binaries/rel(n)).read_bytes();need(sha(b)==m['sha256'] and len(b)==m['bytes'], 'binary identity:'+n)
    gate=doc('full-gate/report.json'); planned=doc('full-gate/commands.json')
    need(gate['source_commit']==SOURCE and gate['passed'] and gate['all_stages_closed'] and gate['all_stages']==len(gate['completed'])==len(planned)==55 and [r['command']for r in gate['completed']]==planned, 'full gate inventory')
    for r in gate['completed']:
        need(r['exit_code']==0 and sha(records['full-gate/'+rel(r['log'])])==r['log_sha256'], 'full gate stage')
    inventories=[re.search(rb'(\d+) tests run:\s*(\d+) passed,\s*(\d+) skipped', records['full-gate/'+r['log']]) for r in gate['completed']]
    inv=[i for i in inventories if i]
    need(len(inv)==1 and inv[0][1]==inv[0][2]==b'1211' and inv[0][3]==b'0', 'actual nextest census')
    build=doc('release-build/report.json');need(build['source_commit']==SOURCE and build['passed'] and build['all_closed'] and len(build['rows'])==2 and all(r['exit_code']==0 for r in build['rows']), 'release builds')
    campaign=doc('final-runtime/campaign.json');commands=doc('final-runtime/commands.json')
    need(campaign['source_commit']==SOURCE and campaign['all_commands_closed'] and campaign['passed'] and len(campaign['rows'])==len(commands)==12 and [r['command'] for r in campaign['rows']]==commands, 'runtime command inventory')
    for r in campaign['rows']:
        need(r['exit_code']==0 and sha(records['final-runtime/'+rel(r['log'])])==r['log_sha256'], 'runtime stage')
    for n,total in [(9,21),(10,16)]:
        log=records[f'final-runtime/campaign-{n:02}.log']
        need("'--dive-bin'" in commands[n][-1] and b'workers [1, 4, 8]' in log and f'gate: {total} of {total} models run'.encode() in log and b'gate: passed' in log and len(re.findall(rb'\[\d+/\d+\] agree',log))==total, 'actual conformance controls')
    for folder,reference,total,expected in [('raw-main242','reference',242,{'exact-raw-ast':202,'named-refusal':26,'rejection-accounted':10,'command-rejection-uncredited':2,'primary-invalid-raw-uncredited':2}),('raw-none56','nof-reference',56,{'exact-raw-ast':56})]:
        r=doc('final-runtime/'+folder+'/report.json');primary=doc(reference+'/primary/manifest.json');by={e['id']:e for e in primary['rows']}
        need(r['selected']==len(r['rows'])==total and r['summary']==expected and r['family_certified'] is False and r['native_binary_sha256']==d['binary_sha256']['context'] and set(by)=={x['id'] for x in r['rows']}, 'raw census:'+folder)
        for row in r['rows']:
            out=row.get('document')
            if row['outcome']=='command-rejection-uncredited':need(out is None,'command rejection credited');continue
            need(out['artifact_admitted'] is False and out['model_proved'] is False and out.get('exhaustive',False) is False,'raw false proof')
            if row['outcome']=='named-refusal':
                error=out['checksum']['Err']['diagnostic'];need(error['code']=='Unsupported' and isinstance(error['message'],str) and bool(error['message']), 'raw named refusal')
            if row['outcome'] in ['exact-raw-ast','primary-invalid-raw-uncredited']:
                v=out['checksum']['Ok'];n=row['id'];need(v['serialized_ast'].encode()==records[reference+'/primary/'+n+'.translated.ast'] and v['checksum']==by[n]['stages']['translated']['crc'], 'raw header exactness')
    for folder,total,expected in [('public-main242',484,{'admitted-exact-cst-sany-validator':404,'named-public-refusal':48,'rejection-accounted':20,'command-rejection-uncredited':4,'invalid-primary-uncredited':8}),('public-none56',112,{'admitted-exact-cst-sany-validator':112})]:
        r=doc('final-runtime/'+folder+'/report.json');reference='reference'if folder=='public-main242'else'nof-reference';refs={e['id']:e for e in doc(reference+'/primary/manifest.json')['rows']}
        need({row['id']for row in r['rows']}=={n+'_'+p for n in refs for p in ['dive','tlc']},'complete public selection')
        need(r['selected']==len(r['rows'])==total and r['summary']==expected and r['family_certified'] is False and r['model_proved'] is False and r['native_binary_sha256']==d['binary_sha256']['dive'], 'public census')
        for row in r['rows']:
            if row['outcome']=='command-rejection-uncredited':continue
            stream=packed('final-runtime/'+folder+'/'+row['id']+'.native.stdout.gz');out=json.loads(stream)
            need(sha(stream)==row['native']['stdout_sha256'] and out['model_proved'] is False and out['exhaustive'] is False and out['profile']==row['profile'] and out['exit_code']==row['native']['exit_code'] and out['result']!='internal-error', 'public document claims')
            if row['outcome']=='admitted-exact-cst-sany-validator':
                need(out['accepted'] and out['result']=='replacement' and out['algorithm_checksum_verified'] and out['body_checksum_verified'] and row['native']['exit_code']==0 and row['validator']=={'configured':'[NO_DIVERGENCE]','unconfigured':'[NO_DIVERGENCE]'}, 'public artifact admission')
                need(refs[row['source_id']]['sany']['accepted'] is True, 'primary SANY admission')
                checksum=out['replacement']['algorithm_checksum'];need(checksum['serialized_ast'].encode()==records[reference+'/primary/'+row['source_id']+'.translated.ast'] and checksum['checksum']==refs[row['source_id']]['stages']['translated']['crc'],'public exact header')
                stem='final-runtime/'+folder+'/'+row['id']
                def tree(name):
                    data=json.loads(packed(stem+'.'+name+'-cst.stdout.gz'));need(data['accepted'], 'CST rejected');return data['tree']
                need(tree('native')==tree('primary'), 'whole generated CST')
                module=re.search(rb'MODULE\s+([A-Za-z][A-Za-z0-9_]*)',records[reference+'/cases/'+row['source_id']+'.tla'])[1]
                for label in ['native','primary']:
                    sany=packed(stem+'.'+label+'-sany.stdout.gz')
                    need(b'Semantic processing of module '+module in sany and
                         not any(marker in sany for marker in [b'Semantic errors:',b'Fatal errors:',b'Parsing aborted',b'*** Errors:']), 'actual admitted SANY:'+label)
                for suffix in ['native-configured','native-unconfigured']:
                    need(re.search(rb'VALIDATION\t([^\r\n]*)', packed(stem+'.'+suffix+'.stdout.gz'))[1]==b'[NO_DIVERGENCE]','actual Validator')
            else:need(out['accepted'] is False and out['result'] in ['unsupported','rejected'] and row['native']['exit_code'] in [1,3] and out.get('replacement') is None,'refusal credited')
    models=doc('final-runtime/models/report.json');selection=doc('reference/model-selection.json')
    need(models['selected']==len(models['rows'])==308 and models['selected_source_ids']==selection['source_ids'] and len(selection['source_ids'])==154 and models['worker_store_controls']==3696 and models['valid_witness_controls']==0 and models['rejected_mutants']==616 and not models['family_certified'], 'finite model census')
    need({r['id'] for r in models['rows']}=={n+suffix for n in selection['source_ids'] for suffix in ['.complete','.violation']}, 'finite selection complete')
    for row in models['rows']:
        primary=row['primary'];stream=packed('final-runtime/models/'+primary['stdout']['path']);need(sha(stream)==primary['stdout']['sha256'], 'primary stream')
        counts=re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.',stream)
        need(len(counts)==1 and all(row['primary_counts'][k]==int(v.replace(b',',b'')) for k,v in zip(['generated','distinct','left_on_queue'],counts[0])), 'primary counts')
        depth=re.findall(rb'The depth of the complete state graph search is ([0-9,]+)',stream)
        if depth:need(row['primary_counts']['depth']==int(depth[0].replace(b',',b'')), 'primary depth')
        violation=row['id'].endswith('.violation');need(primary['exit_code']==(12 if violation else 0) and len(row['settings'])==12 and {(r['profile'],r['workers'],r['store'])for r in row['settings']}=={(p,w,s)for p in ['dive','tlc']for w in [1,4,8]for s in ['fp','exact']}, 'model settings')
        for setting in row['settings']:
            capture=setting['capture'];b=packed('final-runtime/models/'+capture['stdout']['path']);out=json.loads(b)
            need(sha(b)==capture['stdout']['sha256'] and out['verdict']==('invariant-violation' if violation else 'success') and all(out['statistics'][k]==v for k,v in row['primary_counts'].items()) and capture['exit_code']==((12 if setting['profile']=='tlc' else 10)if violation else 0),'native model class/count/exit')
        need(len(row['independent_validation'])==(5 if violation else 0),'strict witness controls')
        for control in row['independent_validation']:
            out=json.loads(packed('final-runtime/models/'+control['capture']['stdout']['path']));need(out['result']!='valid' and control['outcome']==('pending-named-refusal'if control['control']=='original'else'rejected'), 'strict saved-frame credit')
    replay=doc('final-runtime/replay/report.json')
    need(replay['selected']==len(replay['rows'])==1540 and replay['qualified_canonical_is_not_tlc_validation'] and replay['summary']=={'dive.valid':154,'dive.invalid':616,'tlc.unsupported':462,'tlc.invalid':308},'separate replay census')
    need({r['id'] for r in replay['rows']}=={n+'.violation.'+p+'.'+c for n in selection['source_ids']for p in ['dive','tlc']for c in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']},'complete replay matrix')
    for row in replay['rows']:
        stream=packed('final-runtime/replay/'+row['id']+'.stdout.gz');out=json.loads(stream)
        need(sha(stream)==row['stdout_sha256'] and out['result']==row['actual_result'] and row['exit_code']=={'valid':0,'invalid':1,'unsupported':3}[out['result']], 'actual replay')
        need(out['result']!='valid' or(row['profile']=='dive'and row['control']=='original'), 'mutant/strict false valid')
    for folder in ['guides-final','guides-actual-markdown']:
        r=doc('final-runtime/'+folder+'/report.json')
        need(r['selected']==len(r['rows'])==20,'guide census')
        for row in r['rows']:
            stream=packed('final-runtime/'+folder+'/'+row['id']+'.stdout.gz');out=json.loads(stream)
            need(sha(stream)==row['stdout_sha256'] and row['outcome']=='admitted-no-model-proof' and out['accepted'] and out['result']=='replacement' and not out['model_proved'] and not out['exhaustive'] and out['body_checksum_verified'],'actual guide')
            n,profile,mode=row['id'].split('.');need(out['profile']==('tlc+overrides'if profile=='tlc'and mode.endswith('body')else profile),'guide profile')
            reference='reference'if folder=='guides-final'else'markdown-guide-base/guide'
            need(sha(records[reference+'/cases/'+n+'.tla'] if folder=='guides-final' else records[reference+'/cases/'+n+'.tla'])==row['source_sha256'],'guide source')
            if mode.endswith('body'):need(not out['algorithm_checksum_verified'],'body-only full claim')
            else:need(out['algorithm_checksum_verified'] and out['replacement']['algorithm_checksum']['serialized_ast'].encode()==records[reference+'/primary/'+n+'.translated.ast'],'guide header')
    crc=doc('final-runtime/crc/report.json');need(crc['selected']==len(crc['rows'])==24 and all(r['model_proved'] is False and r['outcome'] in ['valid-unchanged','corruption-refused']for r in crc['rows']),'CRC controls')
    forgery=doc('final-runtime/forgeries/report.json');need(forgery['selected']==28 and forgery['genuine_controls']==4 and forgery['forged_controls']==24 and all(r['exit_code']==int(r['mutation']!='baseline')for r in forgery['rows']), 'forgery controls')
    repair=doc('committed-assembler-repair/receipt.json')
    need(repair['after_all_native_consumers_closed'] and repair['packaging_only'] and repair['no_runtime_retag'] and repair['syntax_parse_passed'] and repair['exit_code']==0 and repair['selected_settings']==56 and repair['materialized_records']==953 and repair['matches_original_repaired_reference'], 'committed assembler repair')
    before=records['committed-assembler-repair/before.py'];after=records['committed-assembler-repair/after.py']
    need(before==source(PREFIX+'assemble-reference.py') and after==before.replace(b'else242',b'else 242') and sha(before)==repair['before_sha256'] and sha(after)==repair['after_sha256'], 'exact metadata repair');ast.parse(after)
    for n,h in repair['files_sha256'].items():need(sha(records['committed-assembler-repair/materialized/'+rel(n)])==h==sha(records['nof-reference/'+rel(n)]),'complete NoF repair readback')
    prototype=doc('prototype-binary/qualification.json')
    need(prototype['compiled_source_commit'] is None and prototype['compiled_source_archive_complete'] is False, 'prototype retag')
    return {'records':len(records),'selected_source_inputs':6542,'separate_external_inputs':5058,'gate_commands':55,'gate_tests':1211,'runtime_commands':12,'model_settings':3696,'canonical_valid':154,'strict_saved_frame_valid':0,'family_certified':False}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--base',type=Path,default=Path(__file__).resolve().parent);p.add_argument('--binaries',type=Path);a=p.parse_args()
    try:print(json.dumps(verify(a.base,a.binaries),sort_keys=True))
    except (ValueError,KeyError,TypeError,AssertionError,FileNotFoundError)as e:raise SystemExit('Process-procedure delivery verification failed: '+str(e))

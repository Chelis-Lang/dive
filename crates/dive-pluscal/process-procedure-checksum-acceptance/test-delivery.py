#!/usr/bin/env python3
"""Rehash genuine evidence mutations; transport integrity cannot excuse false claims."""
import argparse,base64,copy,gzip,hashlib,importlib.util,json,tempfile
from pathlib import Path
P=argparse.ArgumentParser();P.add_argument('--base',type=Path,default=Path(__file__).resolve().parent);P.add_argument('--out',type=Path);a=P.parse_args()
s=importlib.util.spec_from_file_location('delivery_reader',a.base/'verify-delivery.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
sha=lambda b:hashlib.sha256(b).hexdigest()
d=json.loads((a.base/'delivery.json').read_bytes());raw=(a.base/d['records_archive']).read_bytes();original=json.loads(gzip.decompress(raw));by={r['path']:i for i,r in enumerate(original)}
rows=[]
baseline=m.verify(a.base);rows.append({'control':'baseline','rejected':False,'result':baseline})
controls=[]
def change(label,path,fn,packed=False):controls.append((label,path,fn,packed))
def field(k,v):return lambda x:x.__setitem__(k,v)
change('producer-retag','final-runtime/producer.json',field('source_commit','invented'))
change('source-index-drop','producer/index.json',lambda x:x['inputs'].pop(next(iter(x['inputs']))))
change('external-index-drop','runtime-inputs-declaration.json',lambda x:x['inputs'].pop(next(iter(x['inputs']))))
change('literal-target-gap','literal-include-audit.json',field('missing',['invented']))
change('gate-skipped','full-gate/report.json',lambda x:x['completed'][0].__setitem__('exit_code',1))
change('gate-open','full-gate/report.json',field('all_stages_closed',False))
change('runtime-command-failure','final-runtime/campaign.json',lambda x:x['rows'][4].__setitem__('exit_code',1))
change('main-raw-false-proof','final-runtime/raw-main242/report.json',lambda x:x['rows'][0]['document'].__setitem__('model_proved',True))
change('raw-header-guess','final-runtime/raw-main242/report.json',lambda x:x['rows'][0]['document']['checksum']['Ok'].__setitem__('checksum','00000000'))
change('raw-refusal-credit','final-runtime/raw-main242/report.json',lambda x:x['summary'].__setitem__('exact-raw-ast',242))
change('public-skip','final-runtime/public-main242/report.json',lambda x:x['rows'].pop())
change('public-false-proof','final-runtime/public-main242/MultipleHeader_C_both_dive.native.stdout.gz',field('model_proved',True),True)
change('public-false-exhaustive','final-runtime/public-main242/MultipleHeader_C_both_dive.native.stdout.gz',field('exhaustive',True),True)
change('public-mixed-profile','final-runtime/public-main242/MultipleHeader_C_both_dive.native.stdout.gz',field('profile','tlc'),True)
change('public-false-checksum','final-runtime/public-main242/MultipleHeader_C_both_dive.native.stdout.gz',field('algorithm_checksum_verified',False),True)
change('SANY-semantic-error','final-runtime/public-main242/MultipleHeader_C_both_dive.native-sany.stdout.gz',lambda b:b+b'\nSemantic errors:\n*** Errors: 1\n',packed='text')
change('SANY-primary-semantic-error','final-runtime/public-main242/MultipleHeader_C_both_dive.primary-sany.stdout.gz',lambda b:b+b'\nSemantic errors:\n*** Errors: 1\n',packed='text')
change('whole-CST-changed','final-runtime/public-main242/MultipleHeader_C_both_dive.native-cst.stdout.gz',field('tree',['invented']),True)
change('model-count','final-runtime/models/report.json',lambda x:x['rows'][0]['primary_counts'].__setitem__('generated',999))
change('model-settings-drop','final-runtime/models/report.json',lambda x:x['rows'][0]['settings'].pop())
change('model-verdict','final-runtime/models/MultipleHeader_C_both.complete.dive.w1.fp.stdout.gz',field('verdict','internal-error'),True)
change('strict-saved-frame-credit','final-runtime/models/report.json',field('valid_witness_controls',154))
change('canonical-relabelled-strict','final-runtime/replay/report.json',lambda x:x['rows'][0].__setitem__('profile','tlc'))
change('invalid-mutant-valid','final-runtime/replay/report.json',lambda x:x['rows'][1].__setitem__('actual_result','valid'))
change('guide-false-proof','final-runtime/guides-actual-markdown/ProcessHeader_P_member_parameter.dive.cli-full.stdout.gz',field('model_proved',True),True)
change('crc-proof-claim','final-runtime/crc/report.json',lambda x:x['rows'][0].__setitem__('model_proved',True))
change('forgery-accepted','final-runtime/forgeries/report.json',lambda x:x['rows'][1].__setitem__('exit_code',0))
change('assembler-readback-forgery','committed-assembler-repair/receipt.json',field('matches_original_repaired_reference',False))
prototype=next(n for n in by if n.startswith('prototype-binary/')and n.endswith('.json'))
change('prototype-retag',prototype,field('compiled_source_commit','0064beb1'))
for label,path,fn,packed in controls:
    records=original.copy();r=original[by[path]].copy();data=base64.b64decode(r['data'])
    if packed=='text':data=fn(gzip.decompress(data))
    else:
        obj=json.loads(gzip.decompress(data)if packed else data);fn(obj);data=json.dumps(obj,separators=(',',':')).encode()
    if packed:data=gzip.compress(data,mtime=0)
    r.update(data=base64.b64encode(data).decode(),sha256=sha(data));records[by[path]]=r
    # Rehash recorded stream identities as well, so semantic checks must reject
    # forged documents even when both outer and inner transport hashes agree.
    touched={path:data}
    if packed and path.endswith('.stdout.gz') and '.native-cst.' not in path:
        folder=path.rsplit('/',1)[0]; report_path=folder+'/report.json'
        report=json.loads(base64.b64decode(original[by[report_path]]['data']))
        identity=Path(path).name
        for row in report['rows']:
            if identity==row['id']+'.native.stdout.gz':row['native']['stdout_sha256']=sha(gzip.decompress(data))
            if identity==row['id']+'.stdout.gz' and 'stdout_sha256'in row:row['stdout_sha256']=sha(gzip.decompress(data))
            for setting in row.get('settings',[]):
                if setting['capture']['stdout']['path']==identity:setting['capture']['stdout']['sha256']=sha(gzip.decompress(data))
        rdata=json.dumps(report,separators=(',',':')).encode();rr=original[by[report_path]].copy();rr.update(data=base64.b64encode(rdata).decode(),sha256=sha(rdata));records[by[report_path]]=rr;touched[report_path]=rdata
    mutated=gzip.compress(json.dumps(records,separators=(',',':')).encode(),compresslevel=1,mtime=0);claim=copy.deepcopy(d)
    for n,b in touched.items():claim['files_sha256'][n]=sha(b)
    claim['raw_bytes']=d['raw_bytes']+sum(len(b)-len(base64.b64decode(original[by[n]]['data']))for n,b in touched.items());claim['records_archive_sha256']=sha(mutated)
    with tempfile.TemporaryDirectory(prefix='dive-process-header-mutation-')as tmp:
        b=Path(tmp);(b/claim['records_archive']).write_bytes(mutated);(b/'delivery.json').write_text(json.dumps(claim))
        try:m.verify(b)
        except (ValueError,KeyError,TypeError,AssertionError,FileNotFoundError)as e:rows.append({'control':label,'path':path,'rejected':True,'diagnostic':str(e)})
        else:raise SystemExit('Rehashed semantic forgery accepted: '+label)
    print('rejected',label,flush=True)
result={'selected':len(rows),'original_source_commit':d['source_commit'],'records_archive_sha256':sha(raw),'script_sha256':sha(Path(__file__).read_bytes()),'all_mutations_rehashed':True,'all_closed':True,'rows':rows}
if a.out:a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'selected':len(rows),'genuine':1,'rehashed_mutations_rejected':len(controls)}))

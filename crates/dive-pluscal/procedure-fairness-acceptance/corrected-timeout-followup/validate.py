#!/usr/bin/env python3
"""Validate every native artifact with SANY and curated finite model witnesses.

The primary checker runs clean, generated pinned translator output. Native
artifacts use the same configured properties and are checked independently.
No dumpTrace/generation flags alter the primary run.
"""
import argparse
from collections import Counter
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import tempfile

JAR_SHA='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
SAFE=['FairProcedureModifier_P_mixed_process_call_minus', 'FairProcedureModifier_P_mixed_process_call_plus', 'FairProcedureModifier_P_mixed_process_two_minus', 'FairProcedureModifier_P_mixed_process_two_plus', 'FairProcedureModifier_P_mixed_reach_nested_modifiers']
WITNESS=['FairProcedureModifier_P_mixed_process_call_minus', 'FairProcedureModifier_P_mixed_process_call_plus', 'FairProcedureModifier_P_mixed_process_two_minus', 'FairProcedureModifier_P_mixed_process_two_plus', 'FairProcedureModifier_P_mixed_reach_nested_modifiers']
def sha(data):return hashlib.sha256(data).hexdigest()

def capture(command,cwd,timeout,output,name):
    process=subprocess.run(command,cwd=cwd,capture_output=True,timeout=timeout)
    result={'exit_code':process.returncode}
    for stream,content in [('stdout',process.stdout),('stderr',process.stderr)]:
        filename=name+'.'+stream+'.gz';(output/filename).write_bytes(gzip.compress(content,mtime=0))
        result[stream]=filename;result[stream+'_sha256']=sha(content)
    return result,process.stdout,process.stderr

def config(witness):return 'INIT Init\nNEXT Next\n'+('INVARIANT Good\n' if witness else '')

def checking(stdout):
    stats=re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.',stdout)
    if len(stats)!=1:raise ValueError('missing/duplicate TLC final counts')
    generated,distinct,queue=[int(value.replace(b',',b'')) for value in stats[0]]
    depths=re.findall(rb'The depth of the complete state graph search is ([0-9,]+)',stdout)
    return {'generated':generated,'distinct':distinct,'left_on_queue':queue,'depth':int(depths[0].replace(b',',b'')) if depths else None}

def validate(args):
    execution_script_sha=sha(Path(__file__).read_bytes())
    if sha(args.jar.read_bytes())!=JAR_SHA:raise ValueError('oracle hash mismatch')
    selection=json.loads(args.report.read_text())
    if selection['summary'].get('disagree'):raise ValueError('token comparison has unresolved disagreement')
    manifest_path=args.fixtures/'manifest.json'
    manifest=json.loads(manifest_path.read_text())
    comparator=Path(__file__).with_name('compare.py')
    if selection.get('manifest_sha256')!=sha(manifest_path.read_bytes()) or selection.get('comparator_sha256')!=sha(comparator.read_bytes()) or selection.get('native_sha256')!=sha(args.example.read_bytes()) or selection.get('model_proved') is not False:
        raise ValueError('artifact selection provenance/proof metadata mismatch')
    ids=[row['id'] for row in selection['rows']]
    expected=[row['id'] for row in manifest['rows']]
    if len(ids)!=len(set(ids)) or set(ids)!=set(expected) or selection.get('selected')!=len(ids) or selection['summary']!=dict(Counter(row['outcome'] for row in selection['rows'])):
        raise ValueError('complete artifact selection/accounting mismatch')
    spec=importlib.util.spec_from_file_location('pluscal_artifact_comparator',comparator)
    comparison=importlib.util.module_from_spec(spec);spec.loader.exec_module(comparison)
    observed={row['id']:row['outcome'] for row in selection['rows']}
    flags={row['id']:row['options'] for row in manifest['rows']}
    for row in manifest['rows']:
        source=(args.inputs/row['input']).read_bytes()
        if sha(source)!=row['input_sha256']:raise ValueError('source integrity mismatch')
        comparison.verify(args.fixtures,row['primary'])
        if 'sany' in row:comparison.verify(args.fixtures,row['sany'])
        fresh=comparison.outcome(row,comparison.execute(args.example,source,row['options']),source,args.fixtures,args.example)
        if fresh['outcome']!=observed[row['id']] or fresh['outcome']=='disagree':raise ValueError('artifact selection masked a fresh outcome '+row['id'])
    rows=[row for row in selection['rows'] if row['outcome']=='token-artifact-match']
    if not rows:raise ValueError('empty generated artifact validation')
    args.output.mkdir(parents=True,exist_ok=True)
    java=[str(args.java),'-Xmx256m','-cp',str(args.jar)]
    results=[];models=[]
    for row in rows:
        name=row['id'];source=(args.inputs/(name+'.tla')).read_bytes()
        with tempfile.TemporaryDirectory(prefix='dive-native-pluscal-') as temporary:
            work=Path(temporary);(work/(name+'.tla')).write_bytes(source)
            api,stdout,_=capture([str(args.native),'translate',name+'.tla','--compat=tlc','--output','json',*flags[name]],work,20,args.output,name+'.native')
            doc=json.loads(stdout)
            if api['exit_code']!=0 or doc['result']!='artifact' or doc['accepted'] is not True or doc['model_proved'] is not False or doc['exhaustive'] is not False or doc['analysis']['accepted'] is not True:
                raise ValueError('native API did not admit a validated nonproof artifact: '+name)
            module=doc['module_source'].encode();path=name+'.native.tla';(args.output/path).write_bytes(module)
            (work/(name+'.tla')).write_bytes(module)
            sany,_,_=capture(java+['tla2sany.SANY','-error-codes',name+'.tla'],work,60,args.output,name+'.native-sany')
            primary=(args.fixtures/(name+'.translated.tla')).read_bytes()
            structures=[]
            for artifact in (primary,module):
                run=subprocess.run([str(args.example),'--cst'],input=artifact,capture_output=True,timeout=20)
                if run.returncode!=0: raise ValueError('CST dumper failed '+name)
                parsed=json.loads(run.stdout)
                if parsed.get('accepted') is not True: raise ValueError('CST parser rejected admitted artifact '+name)
                structures.append(parsed['structure'])
            structure_equal=structures[0]==structures[1]
            outcome='native-sany-match' if sany['exit_code']==0 and structure_equal else 'disagree' 
            results.append({'id':name,'outcome':outcome,'artifact_sha256':sha(module),'artifact':path,'api':api,'sany':sany,'operator_structure_equal':structure_equal,'operator_structure_sha256':sha(json.dumps(structures[1],ensure_ascii=False).encode())})
            if outcome!='native-sany-match':continue
            for witness in [False,True] if name in WITNESS else [False]:
                if name not in SAFE:continue
                deadlock=not witness and name.endswith('_missing_return')
                label=name+('.violation' if witness else '.deadlock' if deadlock else '.complete')
                native_source=module
                primary=(args.fixtures/(name+'.translated.tla')).read_bytes()
                if witness:
                    suffix=(b'\nGood == \\A self \\in ProcSet: pc[self] # "E"\n' if doc['artifact']['processes'] else b'\nGood == pc # "E"\n')
                    native_source=native_source.replace(b'====',suffix+b'====')
                    primary=primary.replace(b'====',suffix+b'====')
                cfg=(config(witness)+doc['artifact']['configuration']).encode();(work/(name+'.cfg')).write_bytes(cfg)
                (work/(name+'.tla')).write_bytes(primary)
                oracle,raw,_=capture(java+['tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-config',name+'.cfg',name+'.tla'],work,30,args.output,label+'.tlc')
                primary_counts=checking(raw)
                if oracle['exit_code']!=(12 if witness else 11 if deadlock else 0):raise ValueError('unexpected primary finite verdict '+label)
                (work/(name+'.tla')).write_bytes(native_source)
                saved=label+'.model.tla';(args.output/saved).write_bytes(native_source)
                cfgpath=label+'.model.cfg';(args.output/cfgpath).write_bytes(cfg)
                implementation,raw,_=capture([str(args.native),'check',name+'.tla','--compat=tlc','--config',name+'.cfg','--seed','0','--fp','0','--output','json'],work,30,args.output,label+'.check')
                result=json.loads(raw);expected='invariant-violation' if witness else 'deadlock' if deadlock else 'success'
                compared=[key for key,value in primary_counts.items() if value is not None]
                agree=implementation['exit_code']==oracle['exit_code'] and result['verdict']==expected and all(result['statistics'].get(key)==primary_counts[key] for key in compared)
                widths=[]
                for workers in (1,4,8):
                    for store in ('fp','exact'):
                        run,out,_=capture([str(args.native),'check',name+'.tla','--compat=tlc','--config',name+'.cfg','--seed','0','--fp','0','--workers',str(workers),'--store',store,'--output','json'],work,30,args.output,label+f'.w{workers}.{store}')
                        data=json.loads(out)
                        matched=run['exit_code']==oracle['exit_code'] and data['verdict']==expected and all(data['statistics'].get(key)==primary_counts[key] for key in compared)
                        if witness or deadlock:matched=matched and data.get('counterexample',{}).get('trace') == result.get('counterexample',{}).get('trace')
                        widths.append({'workers':workers,'store':store,'outcome':'match' if matched else 'disagree','capture':run})
                        agree=agree and matched
                validations=[]
                if witness or deadlock:
                    trace=result['counterexample']['trace'];(work/'trace.json').write_text(json.dumps(trace))
                    for control in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']:
                        candidate=json.loads(json.dumps(trace))
                        if control=='invalid-init':candidate['states'][0]['values'].pop('x',None)
                        elif control=='invalid-next':candidate['states'][-1]['values']['pc']={}
                        elif control=='invalid-closing':candidate['states'].append(json.loads(json.dumps(candidate['states'][0])))
                        elif control=='missing-final-variable':candidate['states'][-1]['values'].pop('x',None)
                        (work/'trace.json').write_text(json.dumps(candidate))
                        replay,out,_=capture([str(args.native),'validate-trace',name+'.tla','--config',name+'.cfg','--trace','trace.json','--compat=tlc','--seed','0','--fp','0','--output','json'],work,20,args.output,label+'.'+control)
                        document=json.loads(out) if out else None
                        valid=document is not None and document.get('result')=='valid'
                        passed=(replay['exit_code']==0 and valid) if control=='original' else (replay['exit_code']!=0 and not valid)
                        validations.append({'control':control,'passed':passed,'capture':replay})
                        agree=agree and passed
                models.append({'id':label,'expected_verdict':expected,'outcome':'finite-model-match' if agree else 'disagree','primary':oracle,'native':implementation,'primary_counts':primary_counts,'compared':compared,'independent_validation':validations,'workers_stores':widths,'source':saved,'source_sha256':sha(native_source),'config':cfgpath,'config_sha256':sha(cfg)})
    report={'schema_version':1,'stage':'native-generated-artifact-controls','oracle_sha256':JAR_SHA,'selection_report_sha256':sha(args.report.read_bytes()),'script_sha256':execution_script_sha,'native_sha256':sha(args.native.read_bytes()),'cst_dumper_sha256':sha(args.example.read_bytes()),'selected_artifacts':len(rows),'artifact_summary':dict(Counter(row['outcome'] for row in results)),'model_summary':dict(Counter(row['outcome'] for row in models)),'family_certified':False,'artifact_rows':results,'model_rows':models}
    (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({key:report[key] for key in ['artifact_summary','model_summary']}))
    return int(report['artifact_summary'].get('disagree',0)+report['model_summary'].get('disagree',0)>0)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['java','jar','native','example','report','inputs','fixtures','output']:parser.add_argument('--'+name,type=Path,required=True)
    raise SystemExit(validate(parser.parse_args()))

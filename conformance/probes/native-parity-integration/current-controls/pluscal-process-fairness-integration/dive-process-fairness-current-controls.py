#!/usr/bin/env python3
"""Fresh combined-source controls; never rewrite ancestral reports."""
import argparse, collections, gzip, hashlib, json, os, re, subprocess, tempfile, time
from pathlib import Path
ROOT=Path('/Users/jeff/code/dive-pluscal-process-shipping-work')
SOURCE='1d0acf0f1916c1156010e296e6c26ea9c1fff759'
TARGET=Path('/Users/jeff/code/dive-checkpoint-keys-shipping-target')
CACHE=Path('/Users/jeff/code/dive-shipping-cache')
ARTIFACTS=Path('/Users/jeff/code/dive-process-fairness-current-artifacts')
OUT=ROOT/'conformance/probes/native-parity-integration/current-controls/pluscal-process-fairness-integration'
JAR_SHA='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
JAR=CACHE/'sha256'/JAR_SHA/'tla2tools.jar'
JAVA=CACHE/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def emit(path,data): path.write_text(json.dumps(data,indent=2)+'\n')
def check_source():
    head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    assert head==SOURCE,head
    subprocess.run(['git','diff','--exit-code',SOURCE,'--','*.rs','Cargo.toml','Cargo.lock'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
def run(label,args,expected=0,env=None):
    check_source(); start=time.time()
    log=OUT/(label+'.log'); command=[str(a) for a in args]
    with log.open('wb') as stream: p=subprocess.run(command,cwd=ROOT,env=env,stdout=stream,stderr=subprocess.STDOUT)
    emit(OUT/(label+'.command.json'),{'source_commit':SOURCE,'command':command,'exit_code':p.returncode,'expected_exit_code':expected,'elapsed_seconds':time.time()-start,'log_sha256':sha(log)})
    if p.returncode!=expected: raise RuntimeError(label+' returned '+str(p.returncode)+' expected '+str(expected))
    print(label+' passed',flush=True)
def build():
    OUT.mkdir(parents=True,exist_ok=True); check_source()
    env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(TARGET),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0')
    run('build-workspace',['cargo','build','--workspace','--release','--locked'],env=env)
    run('build-translate-example',['cargo','build','--release','--locked','-p','dive-pluscal','--example','translate'],env=env)
    ARTIFACTS.mkdir(exist_ok=True)
    for name,src in [('dive',TARGET/'release/dive'),('dive-conformance',TARGET/'release/dive-conformance'),('translate',TARGET/'release/examples/translate')]:
        dst=ARTIFACTS/name;dst.write_bytes(src.read_bytes());dst.chmod(0o755)
    emit(ARTIFACTS/'manifest.json',{'source_commit':SOURCE,'binaries':{n:sha(ARTIFACTS/n) for n in ['dive','dive-conformance','translate']}})
def controls():
    OUT.mkdir(parents=True,exist_ok=True);check_source()
    native=ARTIFACTS/'dive'; example=ARTIFACTS/'translate'
    assert json.loads((ARTIFACTS/'manifest.json').read_text())['source_commit']==SOURCE
    study=ROOT/'crates/dive-pluscal/procedure-fairness-acceptance'
    totals=collections.Counter();models=collections.Counter();qual=collections.Counter();selected=0;artifacts=0
    for label,relative in [('original','.'),('corrected-modifiers','corrected-modifiers'),('timeout-followup','timeout-followup'),('corrected-timeout-followup','corrected-timeout-followup')]:
        campaign=study/relative;dest=OUT/label;dest.mkdir(exist_ok=True)
        report=dest/'comparison.json'
        run(label+'-compare',['python3','-I',campaign/'compare.py','--manifest',campaign/'primary/manifest.json','--inputs',campaign/'cases','--native',example,'--output',report])
        old=json.loads((campaign/'validation/report.json').read_text())
        expected=int(old['artifact_summary'].get('disagree',0)+old['model_summary'].get('disagree',0)>0)
        validation=dest/'validation'
        run(label+'-validate',['python3','-I',campaign/'validate.py','--java',JAVA,'--jar',JAR,'--native',native,'--example',example,'--report',report,'--inputs',campaign/'cases','--fixtures',campaign/'primary','--output',validation],expected=expected)
        fresh=json.loads((validation/'report.json').read_text()); assert fresh['artifact_summary'].get('disagree',0)==0
        assert fresh['selected_artifacts']==old['selected_artifacts']
        assert fresh['model_summary']==old['model_summary'],(label,fresh['model_summary'],old['model_summary'])
        original=campaign/'qualified-replay.py';adapted=dest/'qualified-replay-fresh.py'
        code=original.read_text();oldline="base=Path(__file__).resolve().parent;directory=base/'validation'"
        assert code.count(oldline)==1
        adapted.write_text(code.replace(oldline,'directory=Path('+repr(str(validation))+')'))
        emit(dest/'helper-adaptation.json',{'original':str(original.relative_to(ROOT)),'original_sha256':sha(original),'adapted_sha256':sha(adapted),'change':'Only validation directory is redirected to this fresh combined-source run; decisions/assertions remain unchanged.'})
        run(label+'-qualified-replay',['python3','-I',adapted,'--native',native,'--output',dest/'qualified-replay'])
        q=json.loads((dest/'qualified-replay/report.json').read_text());qual.update(q['summary'])
        comparison=json.loads(report.read_text());selected+=comparison['selected'];totals.update(comparison['summary']);artifacts+=fresh['selected_artifacts'];models.update(fresh['model_summary'])
    assert selected==480 and artifacts==302 and sum(models.values())==118,(selected,artifacts,models)
    assert models=={'finite-model-match':59,'disagree':59},models
    assert qual=={'strict-complete-model-match':59,'strict-witness-pending':59},qual
    guides(native)
    env=os.environ.copy();env.update(DIVE_CACHE_DIR=str(CACHE),HARNESS=str(ARTIFACTS/'dive-conformance'))
    run('cm1',['bash','scripts/conformance-gate.sh','conformance/gates/cm1.txt','--workers','1,4,8','--dive-bin',native],env=env)
    env['DIVE_GATE_CORPUS']='validation-smoke'
    run('validation-smoke',['bash','scripts/conformance-gate.sh','conformance/gates/validation-smoke.txt','--workers','1,4,8','--dive-bin',native],env=env)
    emit(OUT/'runtime-summary.json',{'source_commit':SOURCE,'selected':selected,'artifact_rows':artifacts,'artifact_summary':dict(totals),'strict_model_summary':dict(models),'qualified_replay_summary':dict(qual),'family_certified':False,'strict_witness_credit':False})
def guides(native):
    deps=TARGET/'release/deps';libraries=list(deps.glob('libdive_api-*.rlib'))
    # A workspace release build must leave one coherent API artifact, not choose an arbitrary stale rlib.
    assert len(libraries)==1,[p.name for p in libraries]
    library=libraries[0];rows=[];env=os.environ.copy();env.pop('DIVE_COMPAT',None)
    for guide in ['pluscal-process-procedures','pluscal-multiple-procedures','pluscal-procedure-fairness']:
        path=ROOT/'docs'/ (guide+'.md');text=path.read_text()
        modules=re.findall(r'```tla\n(.*?)\n```',text,re.S);rust=re.findall(r'```rust\n(.*?)\n```',text,re.S)
        assert len(modules)==2 and len(rust)==1,guide
        first=re.search(r'MODULE (\w+)',modules[0]).group(1)
        for source in modules:
            name=re.search(r'MODULE (\w+)',source).group(1)
            saved=ROOT/'docs/user/examples'/(name+'.tla');assert saved.read_text().strip()==source.strip(),name
            with tempfile.TemporaryDirectory(prefix='dive-fresh-pluscal-guide-') as tmp:
                work=Path(tmp);(work/(name+'.tla')).write_text(source+'\n')
                for profile in ['dive','tlc']:
                    command=[str(native),'translate',name+'.tla','--output','json']+(['--compat=tlc'] if profile=='tlc' else [])
                    p=subprocess.run(command,cwd=work,capture_output=True,timeout=30,env=env);doc=json.loads(p.stdout)
                    assert p.returncode==doc['exit_code']==0 and doc['result']=='artifact' and doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
                    rows.append(guide_capture(guide,name,profile,'cli',p,source,path,OUT))
                program=rust[0].replace(first+'.tla',name+'.tla');(work/'main.rs').write_text(program)
                compiled=subprocess.run(['rustc','--edition=2024',str(work/'main.rs'),'--extern','dive_api='+str(library),'-L','dependency='+str(deps),'-o',str(work/'guide')],cwd=work,capture_output=True)
                assert compiled.returncode==0,compiled.stderr.decode()
                p=subprocess.run([str(work/'guide')],cwd=work,capture_output=True,timeout=30,env=env);doc=json.loads(p.stdout)
                assert p.returncode==0 and doc['result']=='artifact' and doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
                row=guide_capture(guide,name,'dive','rust-api',p,source,path,OUT);row.update(program_sha256=hashlib.sha256(program.encode()).hexdigest(),executable_sha256=sha(work/'guide'));rows.append(row)
    assert len(rows)==18
    emit(OUT/'guides.json',{'source_commit':SOURCE,'script_sha256':sha(Path(__file__)),'native_sha256':sha(native),'api_library_sha256':sha(library),'api_library_name':library.name,'controls':18,'cli_controls':12,'compiled_rust_controls':6,'rows':rows,'model_proved':False,'family_certified':False})
def guide_capture(guide,name,profile,kind,p,source,path,dest):
    captures={}
    for stream in ['stdout','stderr']:
        data=getattr(p,stream);file=guide+'-'+name+'-'+profile+'-'+kind+'.'+stream+'.gz';(dest/file).write_bytes(gzip.compress(data,mtime=0));captures[stream]={'path':file,'sha256':hashlib.sha256(data).hexdigest()}
    return {'guide':str(path.relative_to(ROOT)),'guide_sha256':sha(path),'module':name,'source_sha256':hashlib.sha256((source+'\n').encode()).hexdigest(),'profile':profile,'kind':kind,'exit_code':p.returncode,'captures':captures,'model_proved':False,'exhaustive':False}
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('stage',choices=['build','controls']);args=parser.parse_args()
    (build if args.stage=='build' else controls)()

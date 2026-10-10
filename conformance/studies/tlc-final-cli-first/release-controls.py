"""Retain release CLI outputs for the complete previously certified positive set."""
import hashlib,json,subprocess,time
from pathlib import Path
root=Path('/Users/jeff/code/dive-tlc-final-cli-work')
binary=Path('/Users/jeff/code/dive-tlc-final-cli-target/release/dive')
out=Path(__file__).resolve().parent/'release-controls';out.mkdir(exist_ok=True)
models=['FinalState','FinalAction','FinalFairness','FinalSafe','ExecMultiple','PhaseLocalRole','PhaseTableau','ExecRevisit']
rows=[]
for name in models:
    model=root/'conformance/probes/tlc-final-check/models'/name
    for width in [1,3,4,8]:
        args=['check','--output','json','--compat','tlc','--lncheck','final','--lasso','tlc','--seed','0','--fp','0','--workers',str(width),'--config',str(model/f'{name}.cfg'),str(model/f'{name}.tla')]
        t=time.monotonic();p=subprocess.run([str(binary),*args],capture_output=True,timeout=120)
        stem=f'{name}.workers-{width}';(out/f'{stem}.stdout').write_bytes(p.stdout);(out/f'{stem}.stderr').write_bytes(p.stderr)
        v=json.loads(p.stdout);safe=name=='FinalSafe'
        assert p.returncode==(0 if safe else 13),(name,width,p.returncode,v)
        assert v['schema_version']==2 and v['options']['profile']=='tlc'
        assert v['options']['workers']['used']==width and v['execution']['capture_completed'] is True
        assert v['options']['ordinary_temporal_schedule_supported'] is False
        if not safe:
            c=v['counterexample'];assert c['validation']=='valid'
            assert v['property']==' and '.join(v['properties'])
            if name=='ExecMultiple':assert v['property']=='P and Q' and c['ending']=='stuttering'
            if name=='FinalState':assert c['ending']=='lasso' and c['back_to']==1
        rows.append({'model':name,'workers':width,'exit':p.returncode,'elapsed_seconds':time.monotonic()-t,'verdict':v['verdict'],'claim':v.get('properties'),'ending':(v.get('counterexample') or {}).get('ending')})
(out/'selection.json').write_text(json.dumps({'models':models,'widths':[1,3,4,8]},indent=2)+'\n')
(out/'outcomes.json').write_text(json.dumps({'rows':rows},indent=2)+'\n')
print('32 release CLI controls passed; full selected positive set retained')

"""Measure concrete word repair on the released Elevator and real-time clock."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time


def sha(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


def store(path, data):
    path.write_text(json.dumps(data, separators=(",", ":")))


def run(root, command):
    mark = time.perf_counter_ns()
    proc = subprocess.run(["/usr/bin/time", "-qf", "%M", "-o", str(root/"rss"), *map(str, command)],
                          capture_output=True, check=False)
    elapsed = (time.perf_counter_ns()-mark)/1e6
    raw = json.loads(proc.stdout)
    return raw, {"wall_ms": round(elapsed, 3), "peak_rss_kib": int((root/"rss").read_text()),
                 "exit": proc.returncode}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("examples", type=Path, help="the pinned Examples specifications directory")
    parser.add_argument("work", type=Path, help="working directory for regenerated words")
    parser.add_argument("--seven-word", type=Path, help="existing seven-floor donor to avoid repeating its cold search")
    args=parser.parse_args()
    d,root=args.dive.resolve(),args.work.resolve()
    root.mkdir(parents=True,exist_ok=True)
    samples=[]; inputs={}
    def check(name,m,c):
        doc,row=run(root,[d,'check',m,'--config',c,'--workers','1','--seed','0','--fp','0','--max-memory','8g','--time-limit','300s','--output','json'])
        samples.append(dict(case=name,verdict=doc['verdict'],counts=doc['statistics'],**row))
        return (doc.get('counterexample') or {}).get('trace')
    def val(name,m,c,word,repair,expected):
        store(root/'input.json',word)
        doc,row=run(root,[d,'validate-trace',m,'--config',c,'--trace',root/'input.json','--seed','0','--fp','0','--output','json',*(['--repair'] if repair else [])])
        rec=dict(case=name,result=doc['result'],failure=doc.get('failure'),**row)
        det=doc.get('repair',{}).copy(); replacement=det.pop('trace',None)
        if det: rec['repair']=det
        if replacement:
            store(root/'replacement.json',replacement)
            rec.update(length=replacement['length'],word_sha256=sha(root/'replacement.json'))
        samples.append(rec); assert expected==doc['result'],rec
        return replacement
    e=args.examples/'MultiCarElevator';m=(e/'Elevator.tla').resolve();old=(e/'ElevatorLivenessLarge.cfg').read_text()
    for n in [7,8]:
        (root/f'n{n}.cfg').write_text(old.replace('FloorCount = 3',f'FloorCount = {n}'))
    c7=root/'n7.cfg';c8=root/'n8.cfg'
    donor=read(args.seven_word) if args.seven_word else check('cold-seven',m,c7)
    assert donor['length']==249
    val('same-elevator',m,c7,donor,True,'valid')
    val('old-seven-on-eight',m,c8,donor,False,'invalid')
    store(root/'seven.json',donor)
    coords=[1,3,4,5,6,7,8]
    from importlib.util import spec_from_file_location,module_from_spec
    spec=spec_from_file_location("lift",Path(__file__).with_name('elevator-lift-word.py'))
    lift=module_from_spec(spec);spec.loader.exec_module(lift)
    mark=time.perf_counter_ns();anchors=lift.lift(donor,coords,anchors_only=True)
    samples.append({'case':'map-seven-anchors','wall_ms':round((time.perf_counter_ns()-mark)/1e6,3)})
    val('mapped-anchors',m,c8,anchors,False,'invalid')
    word=val('repair-eight',m,c8,anchors,True,'valid')
    assert word and word['length']==262
    val('fresh-eight',m,c8,word,False,'valid')
    word['ending']['back_to']=0
    val('damaged-eight',m,c8,word,False,'invalid')
    inputs.update({f.name:sha(f) for f in [m,c7,c8,root/'seven.json']})
    source=args.examples/'SpecifyingSystems/RealTime'; name='MCRealTimeHourClock'
    m=(source/(name+'.tla')).resolve();c=(source/(name+'.cfg')).resolve()
    inputs.update({f.name:sha(f) for f in [m,c,source/'HourClock.tla']})
    donor=check('cold-original-clock',m,c)
    assert donor['length']==2
    val('same-clock',m,c,donor,True,'valid')
    edited=m.read_text().replace('r > now','r = now + 1')
    for mode in ['slower','fairness','fixed','stale']:
        sub=root/mode;sub.mkdir(exist_ok=True);shutil.copy(source/'HourClock.tla',sub)
        text,cfg=edited,c.read_text()
        if mode=='fairness':
            text=text.replace('ErrorTemporal ==',r"FairSpec == BigInit /\ [][BigNext]_<<hr,now,t>> /\ WF_<<hr,now,t>>(BigNext /\ hr' # hr)"+'\nErrorTemporal ==')
            cfg=cfg.replace('INIT BigInit\nNEXT BigNext','SPECIFICATION FairSpec')
        if mode=='fixed': text=text.replace('[]((now # 4) => <>[](now # 4))','[]TRUE')
        if mode=='stale': cfg=cfg.replace('PROPERTY ErrorTemporal','PROPERTY NonZeno')
        mod,conf=sub/m.name,sub/c.name;mod.write_text(text);conf.write_text(cfg)
        inputs.update({f'{mode}/{f.name}':sha(f) for f in [mod,conf]})
        if mode=='slower':
            val('broken-clock',mod,conf,donor,False,'invalid')
            word=val('repair-clock',mod,conf,donor,True,'valid')
            assert word and word['length']==4
            val('fresh-clock',mod,conf,word,False,'valid')
            word['states'][1]['values']['now']=5
            val('damaged-clock',mod,conf,word,False,'invalid')
        else:
            assert val(mode,mod,conf,donor,True,'invalid') is None
            if mode=='fixed': assert check('ordinary-fallback-fixed',mod,conf) is None
    print(json.dumps({'binary_sha256':sha(d),'input_sha256':inputs,'samples':samples},indent=2))

if __name__=='__main__':
    main()

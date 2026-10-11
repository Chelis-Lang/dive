#!/usr/bin/env python3
"""Redirect retained negative-control outputs without changing their decisions."""
import importlib.util,json
from pathlib import Path
spec=importlib.util.spec_from_file_location('fresh_controls','/tmp/dive-process-fairness-current-controls.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
study=m.ROOT/'crates/dive-pluscal/procedure-fairness-acceptance';m.OUT.mkdir(parents=True,exist_ok=True)
def adapt(name,changes,args):
    original=study/name;code=original.read_text()
    for old,new in changes:
        assert code.count(old)==1,(name,old,code.count(old));code=code.replace(old,new)
    adapted=m.OUT/('fresh-'+name);adapted.write_text(code)
    m.emit(m.OUT/(name+'.adaptation.json'),{'original':str(original.relative_to(m.ROOT)),'original_sha256':m.sha(original),'adapted_sha256':m.sha(adapted),'changes':[{'old':a,'new':b} for a,b in changes],'assertions_weakened':False,'historical_reports_changed':False})
    m.run(name,['python3','-I',adapted,*args])
here='HERE=Path('+repr(str(study))+')';base='BASE=Path('+repr(str(study))+')'
adapt('test-comparison.py',[("HERE=Path(__file__).resolve().parent",here),("HERE/'mutation-controls.json'","Path("+repr(str(m.OUT/'mutation-controls.json'))+")")],['--native',m.ARTIFACTS/'translate'])
adapt('test-timeout-comparison.py',[("BASE=Path(__file__).resolve().parent",base),("BASE/'timeout-mutation-controls.json'","Path("+repr(str(m.OUT/'timeout-mutation-controls.json'))+")")],['--native',m.ARTIFACTS/'translate'])
adapt('check-admission.py',[("HERE=Path(__file__).resolve().parent",here),("output=HERE/'admission'","output=Path("+repr(str(m.OUT/'admission'))+")")],['--native',m.ARTIFACTS/'dive'])
adapt('check-api-admission.py',[("BASE=Path(__file__).resolve().parent",base),("out=BASE/'api-admission'","out=Path("+repr(str(m.OUT/'api-admission'))+")"),("deps=args.target/'debug/deps';library=max(deps.glob('libdive_api-*.rlib'),key=lambda p:p.stat().st_mtime_ns)","deps=args.target/'release/deps';libraries=list(deps.glob('libdive_api-*.rlib'));assert len(libraries)==1;library=libraries[0]")],['--target',m.TARGET])

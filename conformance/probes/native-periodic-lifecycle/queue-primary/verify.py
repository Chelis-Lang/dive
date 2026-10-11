from pathlib import Path
import json,hashlib,itertools
p=Path(__file__).resolve().parent
capture=json.loads((p/'capture.json').read_text())
receipt=json.loads((p/'independent-readback.json').read_text())
for rel,sha in receipt['artifacts_sha256'].items():
    assert hashlib.sha256((p/rel).read_bytes()).hexdigest()==sha,rel
assert [step['exit'] for step in capture['steps']]==[0,0]
rows=[json.loads(line) for line in (p/'stage1.stdout').read_text().splitlines()]
selection=list(itertools.product((1,4,8),range(8),('resume','finished-before','finished-during')))
assert len(rows)==len(selection)==72
for row,(width,repeat,scenario) in zip(rows,selection):
    assert (row['width'],row['repeat'],row['scenario'])==(width,repeat,scenario)
    resume=scenario=='resume'
    assert row['suspended']==resume
    assert row['waiting']==(width if resume else 0)
    assert row['states_taken']==(width if resume else 0)
    assert row['size']==(0 if resume else width)
assert (p/'stage0.stderr').read_bytes()==b''
assert (p/'stage1.stderr').read_bytes()==b''
print('72 retained primary queue lifecycle controls verified; no ordinary schedule proof')

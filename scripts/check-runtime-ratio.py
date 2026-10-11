"""Verify every actual pinned updater observation and generated Rust row."""
from pathlib import Path
import hashlib,json,math,re,struct
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/runtime-ratio'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
for name,digest in load(BASE/'artifact-sha256.json').items():
 path=Path(name);assert not path.is_absolute()and '..'not in path.parts
 assert sha(BASE/path)==digest,name
capture=load(BASE/'capture.json')
assert capture['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert capture['oracle_revision']=='94d0c5070852b37b801bb8998c436383ab7ee37b'and capture['runtime_exit']==0
for name,digest in capture['source_sha256'].items():assert sha(BASE/name)==digest,name
readback=load(BASE/'independent-readback.json');assert readback['stdout_sha256']==sha(BASE/'stage1.stdout')
wrap=lambda v:((v+(1<<63))%(1<<64))-(1<<63)
flt=lambda bits:struct.unpack('>d',bits.to_bytes(8,'big'))[0]
bits=lambda v:0x7ff8000000000000 if math.isnan(v)else int.from_bytes(struct.pack('>d',v),'big')
def evaluate(old,start,interval,delta,now):
 total=wrap(wrap(wrap(now-start)-interval)-delta)
 product=float(total)*old;absolute=product if math.isnan(product)or product>0 else 0.0
 numerator=float(delta)+absolute;denominator=float(wrap(wrap(total+interval)+delta))
 if denominator==0.0:
  if math.isnan(numerator)or numerator==0.0:return bits(float('nan'))
  return bits(math.copysign(float('inf'),numerator*math.copysign(1.0,denominator)))
 return bits(numerator/denominator)
rows=[];negative=[]
for line in(BASE/'stage1.stdout').read_text().splitlines():
 fields=line.split('\t')
 if fields[0]=='NEGATIVE':assert fields[2]=='true';negative.append(int(fields[1]));continue
 assert fields[0]=='ROW'and len(fields)==9
 _,id,old,start,interval,delta,before,after,result=fields
 start,interval,delta,before,after=map(int,(start,interval,delta,before,after));old=int(old,16);result=int(result,16)
 assert 0<=after-before<=1000
 matches=[now for now in range(before,after+1)if evaluate(flt(old),start,interval,delta,now)==result]
 assert matches
 rows.append(dict(id=int(id),previous_bits=old,start_time=start,progress_interval=interval,delta=delta,before=before,after=after,result_bits=result,matching_clock_samples=matches))
assert len(rows)==196 and [r['id']for r in rows]==list(range(196))and rows==readback['rows']
assert negative==readback['negative_assertion_controls']==[-1,-60000,-(1<<63)]
fixture=(ROOT/'crates/dive-liveness/tests/fixtures/runtime_ratio_cases.rs').read_text().split('= &[',1)[1]
actual=[tuple(int(x.strip())for x in row.split(',')if x.strip())for row in re.findall(r'\(([^()]*)\)',fixture)]
assert actual==[tuple(r[k]for k in['previous_bits','start_time','progress_interval','delta','before','after','result_bits'])for r in rows]
failed=load(BASE/'development-missing-directory/failure.json');assert failed['java_exit']==1 and failed['rows']==0
for name,digest in failed['artifact_sha256'].items():assert sha(BASE/'development-missing-directory'/name)==digest,name
for name,digest in load(BASE/'current-source.json')['source_sha256'].items():assert sha(ROOT/name)==digest,name
example=(ROOT/'crates/dive-api/examples/periodic_controller.rs').read_text().split('\n',1)[1].rstrip()
assert '```rust\n'+example+'\n```'in(ROOT/'docs/ordinary-periodic-controller.md').read_text()
print('Verified196 actual updater rows,3 negative assertion controls and every Rust fixture; no ordinary clock/model-proof credit')

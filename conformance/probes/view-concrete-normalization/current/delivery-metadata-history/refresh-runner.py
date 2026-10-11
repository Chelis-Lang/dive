from pathlib import Path
import json,hashlib,gzip,subprocess,shutil
r=Path('/Users/jeff/code/dive-view-normalization-work');b=r/'conformance/probes/view-concrete-normalization/current';a=Path('/Users/jeff/code/dive-view-normalization-artifacts');sha=lambda v:hashlib.sha256(v).hexdigest();history=b/'delivery-metadata-history';history.mkdir(exist_ok=False);shutil.copytree(a/'final-metadata-verification',history/'failed-current-doc-hashes');shutil.copyfile(a/'final-metadata-driver.log',history/'failed-driver.log');shutil.copyfile('/tmp/dive-view-normalization-final-metadata-checks.py',history/'runner.py')
names=['conformance/probes/tlc-planning/combined-context-tableau-source-matrix.json','conformance/probes/tlc-planning/public-ir-admission-source-matrix.json','conformance/probes/tlc-phase-l-graph/source-matrix.json','conformance/probes/predecessor-spill/current-integration-source.json','conformance/probes/native-checkpoint/current-integration-source.json']
names+=['conformance/probes/'+n+'/source-matrix.json'for n in['tlc-final-check','tlc-state-if','tlc-temporal-if','tlc-temporal-if-final','tlc-final-scopes','tlc-phase-execution','temporal-forwarding','tlc-final-property-origins','material-capture','temporal-operators','native-temporal-operators']];names.append('conformance/probes/native-parity-integration/current-source.json');rows=[];changed={}
for name in names:
 p=r/name;old=p.read_bytes();d=json.loads(old);replaced={}
 for f,h in d['source_sha256'].items():
  now=sha((r/f).read_bytes())
  if h!=now:
   original=subprocess.check_output(['git','show','060489e7845b38b894d67b58c9a8c3a9fa18094c:'+f],cwd=r);assert sha(original)==h,(f,h);assert not f.endswith('.rs')and f not in ['Cargo.toml','Cargo.lock'],f
   q=history/'changed-source'/(f+'.gz');q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(gzip.compress(original,mtime=0));changed[f]=h;d['source_sha256'][f]=now;replaced[f]=dict(before=h,after=now)
 q=history/'matrices'/name;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(old);rows.append(dict(path=name,before_sha256=sha(old),changed_source=replaced));p.write_text(json.dumps(d,indent=2)+'\n')
# Refresh only existing generated-manifest digests; preserve all parent members.
p=r/'conformance/probes/native-parity-integration/current-source.json';d=json.loads(p.read_text())
for f in d['manifest_sha256']:d['manifest_sha256'][f]=sha((r/f).read_bytes())
p.write_text(json.dumps(d,indent=2)+'\n');(history/'index.json').write_text(json.dumps(dict(parent_commit='060489e7845b38b894d67b58c9a8c3a9fa18094c',changed_source_sha256=changed,matrices=rows,scope='Frozen060 docs/map bytes preserved; refresh existing current metadata hashes only, no Rust/Cargo changes or runtime retagging'),indent=2)+'\n')
(a/'final-metadata-verification').rename(a/'before-final-metadata-verification');(a/'final-metadata-driver.log').rename(a/'before-final-metadata-driver.log');files={str(q.relative_to(b)):sha(q.read_bytes())for q in b.rglob('*')if q.is_file()and q.name!='index.json'}
# Include nested indexes too; only exclude the current root index itself.
files={str(q.relative_to(b)):sha(q.read_bytes())for q in b.rglob('*')if q.is_file()and q!=b/'index.json'};(b/'index.json').write_text(json.dumps(dict(files_sha256=files),indent=2)+'\n');print('metadata changed',list(changed),'across',len(rows),'preserved maps')

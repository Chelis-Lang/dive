from pathlib import Path
import json,gzip,hashlib,subprocess
root=Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work');base=Path('/tmp/dive-full-crc-publication-controls');out=Path('/tmp/dive-current-full-publication-cst-complete');out.mkdir(exist_ok=False)
cst=Path('/Users/jeff/code/dive-full-checksum-runtime-publication-artifacts/cst');sha=lambda b:hashlib.sha256(b).hexdigest()
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()=='d7c4f5911a1e68d21a10f67bb3bee0546d6448c6'
selected={}
for folder in ['primary','token-controls','control-shapes','context-controls','expression-controls']:
 d=base/folder
 for row in json.loads((d/'manifest.json').read_text())['rows']:selected[row['id']]=d
report=json.loads((base/'full-controls/report.json').read_text());checked=[]
for row in report['rows']:
 ident=row['id']
 for profile in ['dive','tlc']:
  if row[profile]['outcome']!='full-derivation-and-admission':continue
  capture=gzip.decompress((base/'full-controls'/f'{ident}.{profile}.stdout.gz').read_bytes());doc=json.loads(capture)
  inputs=[(selected[ident]/(ident+'.translated.tla')).read_bytes(),doc['replacement']['module_source'].encode()]
  structures=[]
  for label,data in zip(['primary','native'],inputs):
   p=subprocess.run([str(cst),'--cst'],input=data,capture_output=True,timeout=30)
   stem=out/f'{ident}.{profile}.{label}'
   for stream in ['stdout','stderr']:Path(str(stem)+'.'+stream+'.gz').write_bytes(gzip.compress(getattr(p,stream),mtime=0))
   assert p.returncode==0,(ident,profile,label,p.stderr)
   d=json.loads(p.stdout);assert d['accepted'] is True,(ident,profile,label);structures.append(d['structure'])
  equal=structures[0]==structures[1]
  checked.append({'id':ident,'profile':profile,'primary_sha256':sha(inputs[0]),'native_sha256':sha(inputs[1]),'complete_module_cst_equal':equal})
assert len(checked)==304
receipt={'source_commit':'d7c4f5911a1e68d21a10f67bb3bee0546d6448c6','cst_binary_sha256':sha(cst.read_bytes()),'comparator_source_sha256':sha((root/'crates/dive-pluscal/examples/translate.rs').read_bytes()),'complete_module_pairs':304,'mismatches':[{'id':r['id'],'profile':r['profile']}for r in checked if not r['complete_module_cst_equal']],'rows':checked,'qualification':'Exact node hierarchy and all non-trivia token kind/text; no parenthesis, operator, prime, or expression flattening. No full family certification.'}
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p.read_bytes())for p in out.rglob('*')if p.is_file()};(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Checked304 complete-module CST pairs; mismatches:',receipt['mismatches']);assert not receipt['mismatches'],receipt['mismatches']

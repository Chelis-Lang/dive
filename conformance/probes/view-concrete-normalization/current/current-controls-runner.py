from pathlib import Path
import subprocess,hashlib,json,copy,re,time,itertools
r=Path('/Users/jeff/code/dive-view-normalization-work');a=Path('/Users/jeff/code/dive-view-normalization-artifacts/current-controls');a.mkdir(exist_ok=False);binary=Path('/Users/jeff/code/dive-view-normalization-artifacts/current-binaries/dive');sha=lambda b:hashlib.sha256(b).hexdigest();producer=json.loads((binary.parent/'producer.json').read_text());assert sha(binary.read_bytes())==producer['binaries_sha256']['dive'];primary=Path('/Users/jeff/code/dive-view-normalization-artifacts/current-corrected-primary');refs=json.loads((primary/'capture.json').read_text());by={row['model']:row for row in refs['rows']};rows=[];validation=[];baseline={};started=time.time()
def run(cmd,dir,name):
 p=subprocess.run(cmd,capture_output=True,timeout=30);(dir/(name+'.stdout')).write_bytes(p.stdout);(dir/(name+'.stderr')).write_bytes(p.stderr);return p.returncode,json.loads(p.stdout)
def observable(doc,cross_store=False):
 d=copy.deepcopy(doc);d.pop('elapsed_ms');d['options'].pop('workers')
 if cross_store:
  # These two fields alone describe the selected backend; values checked below.
  d['options'].pop('store');d['options'].pop('fingerprint')
 return d
for model,tlc,store,width,chaos in itertools.product(by,[True,False],['fingerprint','exact'],[1,4,8],[None,19]):
 ident=f'{model}-{("tlc" if tlc else "dive")}-{store}-w{width}-c{chaos}';d=a/ident;d.mkdir();file=primary/model/'Compound.tla';base=[str(binary),'check',str(file),'--seed','7','--fp','0','--workers',str(width),'--store',store,'--output','json'];profile=['--compat=tlc']if tlc else[];extra=['--chaos',str(chaos)]if chaos is not None else[];status,doc=run(base+profile+extra,d,'check');assert doc['options']['profile']==('tlc'if tlc else'dive');assert doc['options']['store']==store;assert doc['options']['workers']['requested']==width;assert doc['options']['workers']['chaos']==chaos;assert doc['output']==[];assert doc['collision']is None
 expected_fp={'polynomial_index':0,'polynomial_source':'flag','scheme':'tlc'if tlc else'dive'}if store=='fingerprint'else None;assert doc['options']['fingerprint']==expected_fp
 unsupported=model=='reverse-mixed'
 if unsupported:
  assert status==3 and doc['verdict']=='unsupported'and'deep normalization'in doc['message'];assert doc['counterexample']is None and doc['statistics']['distinct']==0;relation='explicit-refusal-of-unproved-reference-error'
 if not unsupported:
  assert status==(12 if tlc else 10)and doc['verdict']=='invariant-violation'and doc['property']=='Inv';assert tuple(doc['statistics'][n]for n in['generated','distinct','depth'])==(4,4,4);assert doc['counterexample']['length']==4 and doc['counterexample']['trace']is not None;assert doc['options']['workers']['used']==width and doc['options']['workers']['sequential']is False
  if tlc:
   raw=(primary/model/'stdout').read_text();framed=re.findall(r'@!@!@STARTMSG 2217:4 @!@!@\n[^\n]+\n(.*?)@!@!@ENDMSG 2217',raw,re.S);assert len(framed)==4 and all(s.endswith('\n\n')for s in framed);assert [s['text']for s in doc['counterexample']['states']]==[s[:-1]for s in framed],ident
  relation='clean-primary-verdict-counts-property-trace-length'
 if not unsupported or not tlc:
  shape_refused=model in ['reverse-mixed','mixed-set'];trace=doc['counterexample']['trace']if not unsupported else json.loads((a.parent/'default-witness-template-trace.json').read_text());tracefile=d/'trace.json';tracefile.write_text(json.dumps(trace)+'\n');vcmd=[str(binary),'validate-trace',str(file),'--trace',str(tracefile),'--seed','7','--fp','0','--output','json']+profile;vstatus,vdoc=run(vcmd,d,'validate');assert vstatus==(1 if shape_refused else 0) and vdoc['result']==('invalid'if shape_refused else'valid'),(ident,vdoc)
  if shape_refused:assert vdoc['failure']['check']=='shape'and vdoc['failure']['state']==0 and 'Attempted to compare record'in vdoc['failure']['detail']
  validation.append(dict(case=ident,kind='original',exit=vstatus,validation=vdoc['result'],evidence_scope='decoder-shape-refusal-no-semantic-credit'if shape_refused else'independent-semantic-replay',supplied_candidate_not_checker_witness=unsupported))
  for at in[0,1,3]:
   mutant=copy.deepcopy(trace);mutant['states'][at]['values']['x']=99;f=d/f'mutant-{at}.json';f.write_text(json.dumps(mutant)+'\n');cmd=[str(binary),'validate-trace',str(file),'--trace',str(f),'--seed','7','--fp','0','--output','json']+profile;vstatus,vdoc=run(cmd,d,f'mutant-{at}');assert vstatus==1 and vdoc['result']=='invalid',(ident,at,vdoc);validation.append(dict(case=ident,kind=f'mutant-{at}',exit=vstatus,validation=vdoc['result'],evidence_scope='decoder-shape-refusal-no-mutant-credit'if shape_refused else'independent-semantic-rejection'))
 within=(model,tlc,store);value=observable(doc)
 if within in baseline:assert value==baseline[within],ident
 else:baseline[within]=value
 cross=(model,tlc,'cross-store');value=observable(doc,True)
 if cross in baseline:assert value==baseline[cross],ident
 else:baseline[cross]=value
 rows.append(dict(id=ident,exit=status,verdict=doc['verdict'],comparison=relation));print(len(rows),ident,status,flush=True)
 files={str(p.relative_to(a)):sha(p.read_bytes())for p in d.iterdir()if p.is_file()};rows[-1]['files_sha256']=files
(a/'receipt.json').write_text(json.dumps(dict(runtime_producer=producer,primary_capture_sha256=sha((primary/'capture.json').read_bytes()),source_runner_sha256=sha(Path(__file__).read_bytes()),all_selected_candidates_attempt_independent_validation=True,semantic_valid_originals=96,shape_refused_originals=36,semantic_invalid_mutants=288,shape_refused_mutants=108,reverse_default_candidate_source='successful forward-mixed emitted structured trace; equal mathematical values, not a checker certificate for reverse source',cross_store_allowlist=['options.store','options.fingerprint'],within_store_allowlist=['elapsed_ms','options.workers'],primary_text_framing='Remove exactly the one final MP framing newline from each2217 message body; native state text compared byte-for-byte',rows=rows,validation=validation,total_elapsed_seconds=time.time()-started),indent=2)+'\n');assert len(rows)==144 and len(validation)==528;print('closed144 check/132original96Valid36shapeInvalid/396mutants288semantic108shapeInvalid',flush=True)

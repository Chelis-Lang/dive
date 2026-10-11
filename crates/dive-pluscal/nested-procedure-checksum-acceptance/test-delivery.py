#!/usr/bin/env python3
"""Negative integrity controls, separate from immutable nested-header evidence."""
import base64,gzip,importlib.util,json,shutil,tempfile,unittest
from pathlib import Path
HERE=Path(__file__).resolve().parent
SPEC=importlib.util.spec_from_file_location('delivery',HERE/'verify-delivery.py');MODULE=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(MODULE)
class IntegrityControls(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='dive-nested-header-forgery-');self.base=Path(self.temp.name);shutil.copytree(HERE/'reports',self.base/'reports');shutil.copytree(HERE/'packaging',self.base/'packaging')
  for n in ('delivery.json','runtime-records.json.gz'):shutil.copy2(HERE/n,self.base/n)
  self.d=json.loads((self.base/'delivery.json').read_bytes())
 def tearDown(self):self.temp.cleanup()
 def save(self):(self.base/'delivery.json').write_text(json.dumps(self.d))
 def rows(self):return json.loads(gzip.decompress((self.base/'runtime-records.json.gz').read_bytes()))
 def replace_pack(self,rows):
  raw=gzip.compress(json.dumps(rows).encode(),mtime=0);(self.base/'runtime-records.json.gz').write_bytes(raw);self.d['records_archive_sha256']=MODULE.digest(raw);self.save()
 def rewrite_record(self,name,alter):
  rows=self.rows();e=next(x for x in rows if x['path']==name);raw=base64.b64decode(e['data']);j=json.loads(raw);alter(j);new=json.dumps(j).encode();e['data']=base64.b64encode(new).decode();e['sha256']=MODULE.digest(new);self.d['files_sha256'][name]=e['sha256'];self.d['raw_total_bytes']+=len(new)-len(raw);self.replace_pack(rows)
 def rewrite_report(self,name,alter):
  p=self.base/'reports'/(name+'.json');r=json.loads(p.read_bytes());alter(r);raw=json.dumps(r).encode();p.write_bytes(raw);self.d['reports_sha256'][name+'.json']=MODULE.digest(raw);self.save()
 def rejected(self,pattern):
  with self.assertRaisesRegex(ValueError,pattern):MODULE.verify(self.base)
 def test_baseline(self):self.assertEqual(MODULE.verify(self.base)['gate_tests'],1187)
 def test_corrupt_raw_new_pack_hash(self):
  rows=self.rows();rows[0]['data']='YWx0ZXJlZA==';self.replace_pack(rows);self.rejected('runtime bytes')
 def test_missing_raw_new_pack_hash(self):self.replace_pack(self.rows()[1:]);self.rejected('complete runtime census')
 def test_duplicate_raw(self):
  rows=self.rows();rows.append(rows[0]);self.replace_pack(rows);self.rejected('duplicate runtime')
 def test_unsafe_path(self):
  rows=self.rows();rows[0]['path']='../escape';self.replace_pack(rows);self.rejected('unsafe record path')
 def test_retagged_producer(self):self.d['runtime_source_commit']='invented';self.save();self.rejected('producer qualification')
 def test_family_certified(self):self.d['family_certified']=True;self.save();self.rejected('producer qualification')
 def test_open_consumers(self):self.d['all_consumers_closed']=False;self.save();self.rejected('producer qualification')
 def test_retagged_first_source(self):self.rewrite_record('native-first/producer.json',lambda r:r.update(source_commit=MODULE.FINAL));self.rejected('historical producer')
 def test_false_environmental_gate(self):self.rewrite_record('full-gate-repaired/report.json',lambda r:r.update(passed=True));self.rejected('ENOSPC gate credited')
 def test_false_interrupted_gate(self):self.rewrite_record('full-gate/interrupted.json',lambda r:r.update(gate_passed=True));self.rejected('interrupted gate credited')
 def test_false_completed_campaign(self):self.rewrite_record('native-final/campaign.json',lambda r:r['completed'].pop());self.rejected('complete runtime campaign')
 def test_public_model_proof(self):self.rewrite_report('public-original',lambda r:r.update(model_proved=True));self.rejected('public proof')
 def test_missing_public_row(self):self.rewrite_report('public-original',lambda r:r['rows'].pop());self.rejected('row identity')
 def test_invalid_primary_credited(self):
  def alter(r):next(x for x in r['rows']if x['outcome']=='invalid-primary-uncredited')['outcome']='admitted-exact-cst-sany-validator'
  self.rewrite_report('public-original',alter);self.rejected('disposition census')
 def test_multiline_refusal_credited(self):
  def alter(r):next(x for x in r['rows']if x['outcome']=='named-public-refusal')['outcome']='admitted-exact-cst-sany-validator'
  self.rewrite_report('public-while-lines',alter);self.rejected('disposition census')
 def test_raw_ast_forgery(self):
  def alter(r):r['rows'][0]['document']['checksum']['Ok']['serialized_ast']='forged'
  self.rewrite_report('raw-original',alter);self.rejected('raw assertion')
 def test_strict_witness_forged_valid(self):self.rewrite_report('replay',lambda r:r['summary'].update({'tlc.valid':32}));self.rejected('replay qualification')
 def test_profile_switched_replay_certified(self):self.rewrite_report('replay',lambda r:r.update(qualified_canonical_is_not_tlc_validation=False));self.rejected('replay qualification')
 def test_forgery_accepted(self):self.rewrite_report('forgeries',lambda r:r['rows'][1].update(exit_code=0));self.rejected('forged acceptance')
 def test_missing_model_setting(self):self.rewrite_report('models',lambda r:r['rows'][0]['settings'].pop());self.rejected('model settings')
 def test_refusal_model_credit(self):self.rewrite_report('while-models-bounded',lambda r:r.update(worker_store_controls=144));self.rejected('model scope')
 def test_incomplete_gate(self):self.rewrite_report('full-gate',lambda r:r['completed'].pop());self.rejected('full gate')
if __name__=='__main__':unittest.main()

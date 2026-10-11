#!/usr/bin/env python3
"""Keep negative integrity controls separate from immutable producer evidence."""
import gzip,importlib.util,json,shutil,tempfile,unittest
from pathlib import Path
HERE=Path(__file__).resolve().parent
SPEC=importlib.util.spec_from_file_location('delivery',HERE/'verify-delivery.py');MODULE=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(MODULE)
class IntegrityControls(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='dive-procedure-crc-forgery-');self.base=Path(self.temp.name)
  shutil.copytree(HERE/'reports',self.base/'reports');shutil.copytree(HERE/'packaging',self.base/'packaging')
  for n in ('delivery.json','runtime-records.json.gz'):shutil.copy2(HERE/n,self.base/n)
  self.d=json.loads((self.base/'delivery.json').read_bytes())
 def tearDown(self):self.temp.cleanup()
 def save(self):(self.base/'delivery.json').write_text(json.dumps(self.d))
 def replace_pack(self,rows):
  raw=gzip.compress(json.dumps(rows).encode(),mtime=0);(self.base/'runtime-records.json.gz').write_bytes(raw);self.d['records_archive_sha256']=MODULE.digest(raw);self.save()
 def rewrite_report(self,name,alter):
  p=self.base/'reports'/(name+'.json');r=json.loads(p.read_bytes());alter(r);raw=json.dumps(r).encode();p.write_bytes(raw);self.d['reports_sha256'][name+'.json']=MODULE.digest(raw);self.save()
 def rejected(self,pattern):
  with self.assertRaisesRegex(ValueError,pattern):MODULE.verify(self.base)
 def test_baseline(self):self.assertEqual(MODULE.verify(self.base)['gate_tests'],1183)
 def test_corrupt_raw_with_new_pack_hash(self):
  rows=json.loads(gzip.decompress((self.base/'runtime-records.json.gz').read_bytes()));rows[0]['data']='YWx0ZXJlZA==';self.replace_pack(rows);self.rejected('runtime bytes')
 def test_missing_raw_with_new_pack_hash(self):
  rows=json.loads(gzip.decompress((self.base/'runtime-records.json.gz').read_bytes()));self.replace_pack(rows[1:]);self.rejected('complete runtime census')
 def test_duplicate_raw(self):
  rows=json.loads(gzip.decompress((self.base/'runtime-records.json.gz').read_bytes()));rows.append(rows[0]);self.replace_pack(rows);self.rejected('duplicate runtime')
 def test_unsafe_path(self):
  rows=json.loads(gzip.decompress((self.base/'runtime-records.json.gz').read_bytes()));rows[0]['path']='../escape';self.replace_pack(rows);self.rejected('unsafe record path')
 def test_retagged_producer(self):self.d['source_commit']='invented';self.save();self.rejected('producer qualification')
 def test_family_certified(self):self.d['family_certified']=True;self.save();self.rejected('producer qualification')
 def test_public_model_proof(self):self.rewrite_report('public',lambda r:r.update(model_proved=True));self.rejected('public proof')
 def test_missing_public_row(self):self.rewrite_report('public',lambda r:r['rows'].pop());self.rejected('row identity')
 def test_invalid_primary_credited(self):
  def alter(r):
   row=next(x for x in r['rows']if x['outcome']=='invalid-primary-uncredited');row['outcome']='admitted-exact-cst-sany-validator'
  self.rewrite_report('public',alter);self.rejected('disposition census')
 def test_strict_witness_forged_valid(self):self.rewrite_report('replay',lambda r:r['summary'].update({'tlc.valid':16}));self.rejected('replay qualification')
 def test_profile_switched_replay_certified(self):self.rewrite_report('replay',lambda r:r.update(qualified_canonical_is_not_tlc_validation=False));self.rejected('replay qualification')
 def test_forgery_accepted(self):self.rewrite_report('forgeries',lambda r:r['rows'][1].update(exit_code=0));self.rejected('forged acceptance')
 def test_incomplete_gate(self):self.rewrite_report('full-gate',lambda r:r['completed'].pop());self.rejected('full gate')
if __name__=='__main__':unittest.main()

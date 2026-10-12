"""Separate NoF setting capture; never retroactively part of the main selection."""
import base64,gzip,hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def load():
 index=json.loads((BASE/'nof-context-pack-index.json').read_text());raw=(BASE/'nof-context-pack.json.gz').read_bytes();assert sha(raw)==index['compressed_sha256']
 doc=json.loads(gzip.decompress(raw));assert doc['schema_version']==1 and doc['selected_settings']==56 and doc['new_unique_sources']==0 and doc['native_credit']is False and doc['family_certified']is False
 assert len(doc['records'])==index['record_count']==len(index['records'])==955
 result={}
 for name,r in doc['records'].items():
  assert not Path(name).is_absolute()and'..'not in Path(name).parts
  data=base64.b64decode(r['data'],validate=True);assert sha(data)==r['sha256']==index['records'][name];result[name]=data
 return result

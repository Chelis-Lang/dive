"""Read lossless source archives and exact runtime evidence without retagging provenance."""
from pathlib import Path
import gzip,hashlib,json,tarfile,argparse,re,subprocess
p=argparse.ArgumentParser();p.add_argument('--base',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();base=a.base;root=base.parents[2];receipt=json.loads((base/'receipt.json').read_text());assert receipt['source_commit'] is None and receipt['all_consumers_closed'] and not receipt['family_certified'];artifacts=0
for name,digest in receipt['evidence_sha256'].items():
 assert hashlib.sha256((base/name).read_bytes()).hexdigest()==digest,name;artifacts+=1
sources={}
for candidate in ['candidate3','candidate4','candidate5','candidate6','candidate7']:
 folder=base/'consumer-final'/candidate;producer=json.loads((folder/'producer.json').read_text());source_hashes=producer.get('source_sha256',producer.get('checked_code_and_guide_inputs',{}))
 with tarfile.open(folder/'source.tar.gz','r:gz') as archive:
  for name,digest in source_hashes.items():
   member=archive.extractfile(name+'.gz');assert member is not None,name;data=gzip.decompress(member.read());assert hashlib.sha256(data).hexdigest()==digest,(candidate,name)
 sources[candidate]=len(source_hashes)
current=json.loads((base/'consumer-final/candidate7/producer.json').read_text());equal={};different={};metadata_allowed=set(json.loads((root/'conformance/probes/native-parity-integration/current-source.json').read_text())['generated_manifest_paths'])|{'conformance/parity-features.json'}
for name,digest in current['source_sha256'].items():
 actual=hashlib.sha256((root/name).read_bytes()).hexdigest()
 if actual==digest:equal[name]=actual
 else:
  assert name.endswith('.md') or name in metadata_allowed,name
  different[name]={'runtime_sha256':digest,'delivery_sha256':actual,'qualification':'post-consumer documentation/metadata only; runtime provenance remains unchanged'}
guide_patterns=re.compile(r'```[^\n]*\n(.*?)```',re.S);guide_checks=[]
with tarfile.open(base/'consumer-final/candidate7/source.tar.gz','r:gz') as archive:
 for name in different:
  if name.endswith('.md'):
   original=gzip.decompress(archive.extractfile(name+'.gz').read()).decode();delivery=(root/name).read_text();assert guide_patterns.findall(original)==guide_patterns.findall(delivery),name;guide_checks.append(name)
additional_docs={}
for name in ['docs/pluscal-process-checksums.md','docs/pluscal-process-checksum-options.md','docs/pluscal-source-options.md']:
 original=subprocess.check_output(['git','show','ba024b780252c6e7235ec03db1f2ed1a87cc1b59:'+name],cwd=root);delivery=(root/name).read_bytes();assert guide_patterns.findall(original.decode())==guide_patterns.findall(delivery.decode()),name;additional_docs[name]={'base_sha256':hashlib.sha256(original).hexdigest(),'delivery_sha256':hashlib.sha256(delivery).hexdigest(),'executable_fenced_blocks_unchanged':True,'qualification':'post-consumer prose/link updates; no runtime retag'}
result={'schema_version':1,'source_commit':None,'runtime_source_status':'uncommitted-frozen-source7, original producer unmodified','all_consumers_closed':True,'family_certified':False,'artifact_hashes_checked':artifacts,'archived_source_pairs_checked':sources,'delivery_selected_byte_equal':len(equal),'delivery_metadata_differences':different,'production_rust_and_cargo_byte_equal':True,'runtime_binary_sha256':current['binaries'],'post_consumer_selected_guide_fences_unchanged':guide_checks,'additional_documentation':additional_docs,'delivery_checker_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()};a.out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['delivery_metadata_differences','runtime_binary_sha256']}))

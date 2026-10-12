import gzip,hashlib,json,pathlib,subprocess
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current')
source=json.loads((base/'source-index.json').read_text());producer=source['producer'];assert producer=='7fae387c11675b6a72119cc0013dd18f4eac33cb'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer
out=base/'producer-git-objects';out.mkdir(exist_ok=False);(out/'objects').mkdir();objects={};raws={}
def get(kind,oid):
 if oid in raws:
  assert objects[oid]['kind']==kind;return raws[oid]
 data=subprocess.check_output(['git','cat-file',kind,oid],cwd=root);assert hashlib.sha1((kind+' '+str(len(data))+'\0').encode()+data).hexdigest()==oid
 packed=gzip.compress(data,mtime=0);(out/'objects'/(oid+'.gz')).write_bytes(packed);objects[oid]={'kind':kind,'sha256':hashlib.sha256(packed).hexdigest()};raws[oid]=data;return data
commit=get('commit',producer);tree=commit.splitlines()[0].removeprefix(b'tree ').decode();parsed={}
def entries(oid):
 if oid in parsed:return parsed[oid]
 data=get('tree',oid);items={};start=0
 while start<len(data):
  split=data.index(b' ',start);end=data.index(b'\0',split);mode=data[start:split].decode();name=data[split+1:end].decode();child=data[end+1:end+21].hex();assert len(child)==40 and name not in items;items[name]=(mode,child);start=end+21
 assert start==len(data);parsed[oid]=items;return items
blobs={}
for name,row in source['inputs'].items():
 parts=pathlib.PurePosixPath(name).parts;oid=tree
 for part in parts[:-1]:
  mode,oid=entries(oid)[part];assert mode=='40000'
 mode,blob=entries(oid)[parts[-1]];assert mode in ('100644','100755')
 data=gzip.decompress((base/row['archive']).read_bytes());assert hashlib.sha256(data).hexdigest()==row['sha256'];assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==blob,name;blobs[name]=blob
index={'producer':producer,'tree':tree,'source_index_sha256':hashlib.sha256((base/'source-index.json').read_bytes()).hexdigest(),'objects':objects,'blob_oid':blobs,'qualification':'Portable selected-input Git commit/tree identity. Archived blob bytes reconstruct the recorded Git object IDs; not dependency/toolchain hermeticity or semantic certification.'}
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n');print(json.dumps({'producer':producer,'selected_blobs':len(blobs),'commit_tree_objects':len(objects),'index_sha256':hashlib.sha256((out/'index.json').read_bytes()).hexdigest()}))

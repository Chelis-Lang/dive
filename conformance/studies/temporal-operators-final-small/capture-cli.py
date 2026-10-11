#!/usr/bin/env python3
"""Transparent stdout/stderr archival; outcome authority remains the real binary."""
import hashlib,json,os,signal,subprocess,sys,threading,time
from pathlib import Path
base=Path(__file__).resolve().parent
binary=Path(os.environ['DIVE_CAPTURE_BINARY'])
if sys.argv[1:]==['--version']:
    os.execv(str(binary),[str(binary),*sys.argv[1:]])
args=sys.argv[1:]
key=hashlib.sha256(json.dumps([str(Path.cwd()),args]).encode()).hexdigest()[:16]
raw=base/'raw'/key
raw.parent.mkdir(exist_ok=True)
raw.mkdir(exist_ok=False)
def shown(text):
    return text.replace(os.environ['DIVE_CACHE_DIR'],'$DIVE_CACHE_DIR').replace(str(binary),'$DIVE_BINARY')
meta={'argv':[shown(a) for a in args],'cwd':shown(str(Path.cwd())),'real_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'started_unix_seconds':time.time()}
(raw/'invocation.json').write_text(json.dumps(meta,indent=2)+'\n')
child=subprocess.Popen([str(binary),*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
def relay(stream,path,fd):
    with path.open('wb',buffering=0) as out:
        while block:=os.read(stream.fileno(),65536):
            out.write(block)
            offset=0
            while offset<len(block):offset+=os.write(fd,block[offset:])
threads=[threading.Thread(target=relay,args=(child.stdout,raw/'stdout.txt',1)),threading.Thread(target=relay,args=(child.stderr,raw/'stderr.txt',2))]
for t in threads:t.start()
code=child.wait()
for t in threads:t.join()
(raw/'outcome.json').write_text(json.dumps({'exit_code':code,'elapsed_seconds':time.time()-meta['started_unix_seconds']},indent=2)+'\n')
if code<0:
    signal.signal(-code,signal.SIG_DFL)
    os.kill(os.getpid(),-code)
sys.exit(code)

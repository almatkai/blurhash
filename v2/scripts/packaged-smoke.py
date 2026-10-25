#!/usr/bin/env python3
"""Production extracted executable: hello, initialize, shutdown only; no live helper locating or desktop RPC."""
import json,os,subprocess,uuid
from pathlib import Path
root=Path(__file__).resolve().parents[1];private=root/'.work/package-smoke';private.mkdir(parents=True,exist_ok=True)
binary=os.environ['BLURHASH_SMOKE_BINARY']
p=subprocess.Popen([binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
def send(method,params):
    i='h:'+str(uuid.uuid4());p.stdin.write(json.dumps({'type':'request','id':i,'method':method,'params':params})+'\n');p.stdin.flush();r=json.loads(p.stdout.readline());assert r['type']=='response'and r['id']==i and 'error'not in r;return r['result']
hello=json.loads(p.stdout.readline());assert hello['id']=='com.tezbar.native.blurhash'and hello['protocol']==1 and hello['sdkVersion']=='0.1.1'
assert send('initialize',{'protocol':1,'extensionId':hello['id'],'platform':'macOS','packageDir':str(Path(binary).parent.parent),'dataDir':str(private),'tempDir':str(private),'grantedCapabilities':[],'reason':'test'})['ready']
send('shutdown',{});p.stdin.close();assert p.wait(timeout=5)==0
print('extracted production JSONL hello/initialize/shutdown: passed; no desktop/helper locator used')

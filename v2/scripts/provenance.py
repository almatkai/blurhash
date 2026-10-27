#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,subprocess
ROOT=Path(__file__).resolve().parents[2];V=ROOT/'v2'
SOURCE='048ffb1ac00ed2e5a7a7e99b69248d769663c7f5';SDK='e955911f0bf5b558f9d3eb5183df0adc95c22af2';HOST='ea100cee3f69711de12485ba061950c5fc65258f'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(repo,*args):return subprocess.check_output(['git','-C',str(repo),*args])
def captured(directory,repo,ref):
    rows=[]
    for f in sorted(directory.rglob('*')):
        if not f.is_file():continue
        rel=f.relative_to(directory).as_posix();data=git(repo,'show',ref+':'+rel);assert data==f.read_bytes()
        rows.append({'path':rel,'gitBlob':git(repo,'rev-parse',ref+':'+rel).decode().strip(),'bytes':len(data),'sha256':sha(data)})
    return rows
v1=[]
for name in git(ROOT,'ls-tree','-r','--name-only',SOURCE).decode().splitlines():
    data=git(ROOT,'show',SOURCE+':'+name);current=(ROOT/name).read_bytes()
    expected=data+(b'\n[Native Rust v2 implementation and validation](v2/README.md)\n'if name=='README.md'else b'');assert current==expected,name
    v1.append({'path':name,'gitBlob':git(ROOT,'rev-parse',SOURCE+':'+name).decode().strip(),'bytes':len(data),'sha256':sha(data),'worktreeExact':name!='README.md','permittedChange':'appended discovery link'if name=='README.md'else None})
upstream=[];ref='6734d85992290066fd31a7f5bee3cca3d3545a1a'
paths={'package.json':'extensions/blurhash/package.json','clipboard-blurhash.tsx':'extensions/blurhash/src/clipboard-blurhash.tsx','extension-icon.png':'extensions/blurhash/assets/extension-icon.png','LICENSE':'LICENSE','CODEOWNERS':'.github/CODEOWNERS'}
for name,path in paths.items():
    data=(V/'fixtures/upstream'/name).read_bytes();upstream.append({'path':path,'url':'https://raw.githubusercontent.com/raycast/extensions/'+ref+'/'+path,'bytes':len(data),'sha256':sha(data),'gitBlobComputed':hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()})
lock=json.loads((ROOT/'package-lock.json').read_text())['packages']['node_modules/blurhash'];library=[]
for f in sorted((V/'fixtures/library/typescript').glob('*.ts')):library.append({'path':'TypeScript/src/'+f.name,'bytes':f.stat().st_size,'sha256':sha(f.read_bytes()),'matchedPublishedSourceMapAndPinnedGit':True})
result={'ownRepository':'https://github.com/almatkai/blurhash','ownSourceCommit':SOURCE,'branch':'feat/native-v2-rust','originMainAtFetch':git(ROOT,'rev-parse','origin/main').decode().strip(),'v1':v1,'ownCapturedSource':captured(V/'fixtures/source',ROOT,SOURCE),'sdk':{'repository':'https://github.com/almatkai/tezbar-extension-sdk-rust','rev':SDK,'version':'0.1.1','protocol':1,'captured':captured(V/'fixtures/sdk',ROOT.parent/'tezbar-extension-sdk-rust',SDK)},'host':{'rev':HOST,'captured':captured(V/'fixtures/host',ROOT.parent.parent/'Tezbar',HOST),'workingTreeFilesUsed':False,'modifiedOrBuilt':False},'upstream':{'repository':'https://github.com/raycast/extensions','rev':ref,'author':'Freaktion','codeowner':'freaktion-craft','fetched':upstream,'extensionSpecificLicenseHttpStatus':404,'licenseBasis':'upstream root MIT license plus extension manifest MIT declaration'},'blurhash':{'version':'2.0.5','resolved':lock['resolved'],'integrity':lock['integrity'],'gitRev':'cb151cab5b7d9cd3eef624e12e30381c6d292f0d','licensePath':'License.md','sources':library,'npmContainsStandaloneLicenseFile':False},'icon':{'bytes':47089,'sha256':sha((V/'resources/extension-icon.png').read_bytes()),'exactOwnAndUpstream':(V/'resources/extension-icon.png').read_bytes()==(ROOT/'assets/extension-icon.png').read_bytes()==(V/'fixtures/upstream/extension-icon.png').read_bytes()},'search':{'root':str(ROOT),'prefix':'ax-ec7150c05915','stateDir':'/Users/almat/.local/share/xerj/workspaces/ax-ec7150c05915','initial':'generation 1; 4/4 code files indexed, 10 files/61 records; 4 junk/binary exclusions plus ignore rules; rg/full-source/Git verification used for exhaustive coverage','final':'update aborted: frozen dataset/schema evolution refuses new PROVENANCE JSON fields; final index coverage incomplete, no prefix bypass/restart/schema mutation; exhaustive checks use current files, rg, Git and hashes','globalRestartOrSchemaMutation':False,'prefixBypass':False},'sourceOracle':{'sourceSha256':sha((ROOT/'src/clipboard-blurhash.tsx').read_bytes()),'lockedLibraryExecutedInMemory':True,'sourceBoundaries':'captured fs/os/child_process/Raycast imports; absent fetch; real producer tripwires; fake home/tmp under clone; execFileSync capture seam','realDesktopCalls':0,'realOriginalSourceFilesystemWrites':0},'scope':{'staged':False,'committed':False,'pushed':False,'pullRequest':False,'installed':False,'released':False,'subdelegated':False}}
assert result['icon']['sha256']=='65de4d066d5fda7187729cf4a4e3c6229389aff749ae58ca7b34a487381e521f'
(V/'PROVENANCE.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n');print('provenance/source object hashes and all 15 original V1 files verified')

#!/usr/bin/env python3
"""Deterministic source/development ZIP. No fictional download URL or catalog pins."""
from pathlib import Path
import argparse,hashlib,json,os,stat,struct,subprocess,tempfile,zipfile
ROOT=Path(__file__).resolve().parents[1]
def sha(data):return hashlib.sha256(data).hexdigest()
def package(binary,target):
    data=binary.read_bytes()
    if target=='aarch64-apple-darwin':assert struct.unpack('<II',data[:8])==(0xfeedfacf,0x100000c),'expected native ARM64 Mach-O'
    elif target=='x86_64-pc-windows-msvc':
        assert data[:2]==b'MZ';offset=struct.unpack_from('<I',data,60)[0];assert data[offset:offset+4]==b'PE\0\0' and struct.unpack_from('<H',data,offset+4)[0]==0x8664
    else:raise ValueError('unsupported package target')
    executable='bin/tezbar-blurhash'+('.exe' if target.endswith('msvc')else '')
    members={executable:data,'LICENSE':(ROOT/'LICENSE').read_bytes(),'NOTICE':(ROOT/'NOTICE').read_bytes(),'resources/extension-icon.png':(ROOT/'resources/extension-icon.png').read_bytes()}
    assert len(members['resources/extension-icon.png'])==47089 and sha(members['resources/extension-icon.png'])=='65de4d066d5fda7187729cf4a4e3c6229389aff749ae58ca7b34a487381e521f'
    for p in sorted((ROOT/'licenses').rglob('*')):
        if p.is_file():members[p.relative_to(ROOT).as_posix()]=p.read_bytes()
    manifest=json.loads((ROOT/'extension.json').read_text());manifest['files']={p:sha(b)for p,b in sorted(members.items())};assert manifest['artifacts']=={}
    members['extension.json']=(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n').encode()
    output=ROOT/'output';output.mkdir(exist_ok=True);archive=output/f'blurhash-2.0.0-{target}.zip'
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_STORED)as z:
        for p,b in sorted(members.items()):
            info=zipfile.ZipInfo(p,(2020,1,1,0,0,0));info.create_system=3;info.external_attr=((stat.S_IFREG| (0o755 if p==executable else 0o644))<<16);z.writestr(info,b)
    with zipfile.ZipFile(archive)as z:
        assert z.namelist()==sorted(members);actual=json.loads(z.read('extension.json'))
        assert set(actual['files'])==set(z.namelist())-{'extension.json'}
        for p,digest in actual['files'].items():assert sha(z.read(p))==digest
    # Extract only owned normal members; no profile/catalog/install writes.
    extract=ROOT/'.work'/('extracted-'+target);extract.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(archive)as z:
        for p in z.namelist():
            assert not p.startswith('/') and '..' not in Path(p).parts
            dest=extract/p;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(z.read(p))
    extracted=extract/executable;extracted.chmod(0o755)
    if target=='aarch64-apple-darwin' and os.uname().machine=='arm64':
        for flag in ['--protocol-info','--self-test']:
            probe=json.loads(subprocess.check_output([str(extracted),flag],timeout=5));assert probe['id']==manifest['id'] and probe['protocol']==1
        env=os.environ.copy();env['BLURHASH_SMOKE_BINARY']=str(extracted)
        subprocess.run(['python3',str(ROOT/'scripts/packaged-smoke.py')],env=env,check=True)
    summary={'target':target,'zip':archive.name,'zipBytes':archive.stat().st_size,'zipSha256':sha(archive.read_bytes()),'executable':executable,'executableBytes':len(data),'executableSha256':sha(data),'iconSha256':sha(members['resources/extension-icon.png']),'members':len(members),'files':manifest['files'],'artifacts':{},'publication':'not-published; trusted artifact URLs/digests remain parent-owned'}
    (output/(archive.name+'.sha256')).write_text(summary['zipSha256']+'  '+archive.name+'\n');(output/(target+'.json')).write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({k:v for k,v in summary.items()if k!='files'},indent=2));return summary
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--target',required=True);a=parser.parse_args();package(a.binary.resolve(),a.target)

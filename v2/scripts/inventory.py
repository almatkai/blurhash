#!/usr/bin/env python3
"""All repository Git-visible deliverables, before or after parent staging; no self hash."""
from pathlib import Path
import argparse,hashlib,json,subprocess
ROOT=Path(__file__).resolve().parents[2];SELF='v2/INDEX.json'
parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');args=parser.parse_args()
files=set(subprocess.check_output(['git','-C',str(ROOT),'ls-files','-z','--cached','--others','--exclude-standard']).decode().split('\0'))
selected=sorted(p for p in files if p and p!=SELF)
rows=[]
for name in selected:
    p=ROOT/name;assert p.is_file() and not p.is_symlink(),name
    assert not any(part in ['target','.work','node_modules','output','dist','.git']for part in p.relative_to(ROOT).parts),name
    b=p.read_bytes();rows.append({'path':name,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
value={'schema':1,'scope':'All Git-visible repository files, including unchanged V1; excludes this index and disposable ignored .work/target/output. Pinned original V1 object hashes are also recorded in PROVENANCE.json.','files':rows}
out=ROOT/SELF
if args.check:assert json.loads(out.read_text())==value,'INDEX is stale; regenerate after edits/staging'
else:out.write_text(json.dumps(value,indent=2,ensure_ascii=False)+'\n')
print(f'INDEX {"verified"if args.check else "written"}: {len(rows)} files, {sum(x["bytes"]for x in rows)} bytes')

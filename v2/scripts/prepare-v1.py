#!/usr/bin/env python3
"""Materialize exact original git objects only inside v2/.work/v1."""
from pathlib import Path
import subprocess
root=Path(__file__).resolve().parents[2];target=root/'v2/.work/v1';target.mkdir(parents=True,exist_ok=True)
ref='048ffb1ac00ed2e5a7a7e99b69248d769663c7f5'
for name in subprocess.check_output(['git','-C',str(root),'ls-tree','-r','--name-only',ref],text=True).splitlines():
    out=target/name;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(subprocess.check_output(['git','-C',str(root),'show',ref+':'+name]))
print('exact V1 source/lock/asset copy prepared under v2/.work/v1')

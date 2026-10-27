#!/usr/bin/env python3
"""Verify frozen host excerpts and source schema; generate only clone-owned test modules."""
from pathlib import Path
import difflib, json, subprocess
ROOT=Path(__file__).resolve().parents[1]
HOST=ROOT/'fixtures/host'
def function(text,name):
    import re
    start=re.search(r'^(?:pub(?:\([^\n]*?\))? )?(?:async )?fn '+re.escape(name)+r'\b',text,re.M).start()
    end=text.index('\n}',start)+2
    return text[start:end]+'\n'
def patched(source,patch):
    # Apply only in memory by exact hunk old/new line sequences; no host checkout writes.
    import re
    lines=(ROOT/'integration'/patch).read_text().splitlines(True)
    original=source.splitlines(True);out=[];cursor=0;i=2
    while i<len(lines):
        match=re.match(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@',lines[i]);assert match
        offset=int(match[1])-1;out.extend(original[cursor:offset]);cursor=offset;i+=1
        while i<len(lines) and not lines[i].startswith('@@'):
            line=lines[i];i+=1
            if line.startswith((' ','-')):
                assert original[cursor]==line[1:],(patch,cursor)
                cursor+=1
            if line.startswith((' ','+')):out.append(line[1:])
    out.extend(original[cursor:]);return ''.join(out)

p=(HOST/'src-tauri/src/native_core/extensions/process_install.rs').read_text();l=(HOST/'src-tauri/src/native_core/extensions/launcher.rs').read_text()
v='// Generated exact functions from host ea100cee; see scripts/contracts.py.\n#![allow(dead_code, clippy::redundant_closure, clippy::nonminimal_bool)]\npub mod frozen {\npub mod process_install {\nuse serde_json::Value; use std::{collections::HashSet,path::PathBuf};\n'
v+=p[p.index('const CAPABILITY_GROUPS'):p.index('// Shared by installer')]
v+=function(p,'component')+function(p,'relative')+function(p,'validate')+'}\nmod launcher {\nuse serde_json::Value;use std::collections::HashSet;use super::process_install;\n'+function(l,'bounded_text')+function(l,'validate')+'}\npub fn validate(v:&serde_json::Value)->Result<(),String>{process_install::validate(v)}\n}\n'
# rustfmt preserves upstream semantics; no lint fixes or replacement validator.
formatted=subprocess.run(['rustfmt','--edition','2021'],input=v,text=True,capture_output=True,check=True).stdout

if '--write' in __import__('sys').argv:(ROOT/'tests/support/host_validator.rs').write_text(formatted)
else:assert (ROOT/'tests/support/host_validator.rs').read_text()==formatted,'regenerate frozen validator with --write'
r=(HOST/'src-tauri/src/native_core/extensions/registry.rs').read_text();after=patched(r,'registry-resource-root.patch')
modules=[]
for name,text in [('before',r),('after',after)]:
    struct=text[text.index('pub(super) struct Installed'):text.index('\n}',text.index('pub(super) struct Installed'))+2]
    mod='''#![allow(dead_code,clippy::redundant_closure)]
use serde_json::{json,Value};use std::{collections::HashSet,path::{Path,PathBuf,Component}};
struct Paths {data_dir:PathBuf} struct CoreContext {paths:Paths}
fn read_json(path:&Path)->Result<Value,String>{serde_json::from_slice(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())}
mod process_install {
use super::*;pub struct Package{pub manifest:Value,pub dir:PathBuf}
// Seam: stand-in for successfully VERIFIED Package returned by real installer. Never installs.
pub fn resolve(ctx:&CoreContext,id:&str)->Result<Package,String>{let root=ctx.paths.data_dir.join("extensions/packages").join(id);let a=read_json(&root.join("active.json"))?;let dir=root.join("versions").join(a["directory"].as_str().unwrap());Ok(Package{manifest:read_json(&dir.join("extension.json"))?,dir})}
}
mod registry {use super::*;
'''+struct+'\n'
    for f in ['slug','managed_roots','collect','fallback','icon_path','command_icon_path']:mod+=function(text,f)
    mod+='fn roots(ctx:&CoreContext)->Vec<PathBuf>{managed_roots(ctx)}\n'
    mod+='''pub fn inspect(root:&Path)->(PathBuf,Option<String>,Option<String>){let ctx=CoreContext{paths:Paths{data_dir:root.into()}};let entries=collect(&ctx);let e=entries.iter().find(|e|e.id=="com.tezbar.native.blurhash").unwrap();(e.path.clone(),icon_path(e),command_icon_path(e,&json!({"icon":"resources/extension-icon.png"})))}
}
pub fn inspect(root:&Path)->(PathBuf,Option<String>,Option<String>){registry::inspect(root)}
'''
    mod=subprocess.run(['rustfmt','--edition','2021'],input=mod,text=True,capture_output=True,check=True).stdout
    out=ROOT/f'tests/support/registry_{name}.rs'
    if '--write' in __import__('sys').argv:out.write_text(mod)
    else:assert out.read_text()==mod,'stale registry fixture '+name
print('exact host validator and before/after registry excerpts verified')

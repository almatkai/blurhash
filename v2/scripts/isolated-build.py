#!/usr/bin/env python3
"""Fresh Cargo home + source copy. All writes stay under this clone's v2/.work."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,tempfile
ROOT=Path(__file__).resolve().parents[1]
parent=ROOT/'.work/isolated';parent.mkdir(parents=True,exist_ok=True)
work=Path(tempfile.mkdtemp(prefix='run-',dir=parent));source=work/'source';source.mkdir()
for name in ['Cargo.toml','Cargo.lock']:
    shutil.copyfile(ROOT/name,source/name)
shutil.copytree(ROOT/'src',source/'src',dirs_exist_ok=True)
shutil.copytree(ROOT/'.cargo',source/'.cargo',dirs_exist_ok=True)
manifest=(source/'Cargo.toml').read_text()
assert 'path =' not in manifest.split('[features]')[0]
assert 'https://github.com/almatkai/tezbar-extension-sdk-rust' in manifest
assert 'e955911f0bf5b558f9d3eb5183df0adc95c22af2' in manifest
# QA fixture is a test-only explicit target; production doesn't enable the feature.
assert 'qa-harness' not in os.environ.get('CARGO_FEATURES','')
env=os.environ.copy();env.update(CARGO_HOME=str(work/'cargo-home'),CARGO_TARGET_DIR=str(work/'target'),CARGO_BUILD_JOBS='2')
cmd=['cargo','build','--manifest-path',str(source/'Cargo.toml'),'--release','--locked','--bin','tezbar-blurhash','--target','aarch64-apple-darwin']
subprocess.run(cmd,env=env,cwd=source,check=True)
binary=work/'target/aarch64-apple-darwin/release/tezbar-blurhash'
result={'target':'aarch64-apple-darwin','sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'bytes':binary.stat().st_size,'sdk':'e955911f0bf5b558f9d3eb5183df0adc95c22af2','cargoLockSha256':hashlib.sha256((source/'Cargo.lock').read_bytes()).hexdigest(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'binary':str(binary.relative_to(ROOT)),'sourceDirectory':str(source.relative_to(ROOT)),'freshCargoHome':True,'localPathDependency':False}
(parent/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))

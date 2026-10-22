from pathlib import Path
import hashlib,json,shutil,subprocess,sys
expected=json.loads(Path('/fixtures/source-identities.json').read_text())
for relative,digest in expected.items():
    actual=hashlib.sha256((Path('/source')/relative).read_bytes()).hexdigest()
    if actual != digest:
        raise SystemExit('SOURCE_IDENTITY_MISMATCH '+relative)
print('SOURCE_REQUIRED_IDENTITIES_OK files='+str(len(expected)), flush=True)
print('source_math_lib_sha256='+expected['crates/shortwin-math/src/lib.rs'], flush=True)
print('source_pool_sha256='+expected['crates/shortwin-math/src/pool.rs'], flush=True)
root=Path('/scratch/vesting-native')
root.mkdir()
(root/'src').mkdir()
shutil.copyfile('/fixtures/vesting-native.rs',root/'src/main.rs')
shutil.copyfile('/fixtures/vesting-Cargo.toml',root/'Cargo.toml')
subprocess.run(['/toolchain/bin/rustc','--version','--verbose'],check=True)
result=subprocess.run(['/toolchain/bin/cargo','run','--offline','--manifest-path',str(root/'Cargo.toml'),'--release','--quiet'])
raise SystemExit(result.returncode)

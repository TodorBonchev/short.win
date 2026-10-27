#!/usr/bin/env python3
"""Manual CI only: isolated native SDK compile/test and complete evidence retention.
No oracle endpoint, key, transaction, signing or deployment operations are present.
"""
from __future__ import annotations
import argparse,datetime,hashlib,json,os,platform,shutil,subprocess,sys,tarfile,tomllib
from pathlib import Path

PROBE=Path(__file__).resolve().parent
REPO=PROBE.parent.parent
MANIFEST=PROBE/'Cargo.toml'
LOCK=PROBE/'Cargo.lock'
EXPECTED=json.loads((PROBE/'provenance.json').read_text())

def sha(path:Path)->str:return hashlib.sha256(path.read_bytes()).hexdigest()
def now()->str:return datetime.datetime.now(datetime.timezone.utc).isoformat()
def command(args:list[str],path:Path,*,cwd:Path=PROBE,json_output=False)->dict:
    print('+ '+' '.join(args),flush=True)
    start=now()
    with path.open('wb') as out:
        if json_output:
            with path.with_suffix(path.suffix+'.stderr.log').open('wb') as err:
                proc=subprocess.run(args,cwd=cwd,stdout=out,stderr=err,check=False)
        else:
            proc=subprocess.Popen(args,cwd=cwd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
            assert proc.stdout is not None
            for chunk in iter(lambda:proc.stdout.read(8192),b''):
                out.write(chunk);out.flush();sys.stdout.buffer.write(chunk);sys.stdout.buffer.flush()
            proc.wait()
    return {'argv':args,'cwd':str(cwd),'started_utc':start,'finished_utc':now(),'exit_code':proc.returncode,'output':path.name,'sha256':sha(path)}

def validate_config()->dict:
    data=tomllib.loads(MANIFEST.read_text())
    assert 'workspace'in data and data['workspace']['resolver']=='3'
    toolchain=tomllib.loads((PROBE/'rust-toolchain.toml').read_text())['toolchain']
    assert toolchain['channel']=='1.98.1' and 'targets'not in toolchain
    assert data['package']['publish'] is False
    sb=data['dependencies']['switchboard-on-demand']
    assert sb['version']=='=0.13.0' and sb['default-features'] is False
    assert sorted(sb['features'])==sorted(EXPECTED['feature_contract']['requested'])
    for name,version in EXPECTED['pinned_root_component_versions'].items():
        spec=data['dependencies'][name]
        assert (spec['version'] if isinstance(spec,dict) else spec)=='='+version
    assert LOCK.is_file(), 'pinned standalone Cargo.lock is required; never resolve a replacement silently'
    assert sha(LOCK)==EXPECTED['locked_resolution']['lock_sha256'], 'standalone lock differs from independently verified baseline'
    return data

def retain_sources(evidence:Path,metadata:dict)->dict:
    package_records=metadata['packages'];nodes={n['id']:n for n in metadata['resolve']['nodes']}
    sdk=next(x for x in package_records if x['name']=='switchboard-on-demand' and x['version']=='0.13.0')
    active=nodes[sdk['id']]['features']
    assert not set(active)&set(EXPECTED['feature_contract']['forbidden']),active
    assert set(EXPECTED['feature_contract']['requested'])<=set(active),active
    locks=tomllib.loads(LOCK.read_text())['package']
    lock_checks={(x['name'],x['version']):x.get('checksum') for x in locks}
    sdk_key=('switchboard-on-demand','0.13.0')
    assert lock_checks[sdk_key]==EXPECTED['published_sdk']['archive_sha256']
    cargo_home=Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))
    archives=evidence/'dependency-archives';archives.mkdir(exist_ok=True)
    registry=[];missing=[]
    for package in package_records:
        if package['id']not in nodes or not (package.get('source')or'').startswith('registry+'):
            continue
        name=f"{package['name']}-{package['version']}.crate"
        candidates=list((cargo_home/'registry/cache').glob('*/'+name))
        if not candidates:
            missing.append(name);continue
        expected=lock_checks[(package['name'],package['version'])]
        matching=[x for x in candidates if sha(x)==expected]
        if not matching:raise RuntimeError('registry archive checksum mismatch: '+name)
        shutil.copyfile(matching[0],archives/name)
        registry.append({'file':name,'sha256':expected,'bytes':matching[0].stat().st_size})
    if missing:raise RuntimeError('selected registry sources missing: '+','.join(missing))
    sdk_archive=archives/'switchboard-on-demand-0.13.0.crate'
    assert sha(sdk_archive)==EXPECTED['published_sdk']['archive_sha256']
    file_checks={}
    with tarfile.open(sdk_archive,'r:gz') as tar:
        for relative,expected in EXPECTED['published_sdk']['file_sha256'].items():
            member=tar.extractfile('switchboard-on-demand-0.13.0/'+relative)
            assert member is not None
            raw=member.read();actual=hashlib.sha256(raw).hexdigest();assert actual==expected,(relative,actual)
            file_checks[relative]=actual
    pinned={}
    for name,version in EXPECTED['pinned_root_component_versions'].items():
        assert any(x['name']==name and x['version']==version and x['id']in nodes for x in package_records),(name,version)
        pinned[name]=version
    sdk_node=nodes[sdk['id']]
    sb_program_dep=next(x for x in sdk_node['deps'] if x['name']=='solana_program_v3')
    sb_program=next(x for x in package_records if x['id']==sb_program_dep['pkg'])
    assert (sb_program['name'],sb_program['version'])==('solana-program','4.0.0')
    return {'sdk_features':active,'sdk_source':sdk['source'],'sdk_archive_sha256':sha(sdk_archive),'sdk_file_sha256':file_checks,'pinned_components':pinned,'actual_sdk_bridge_package':{'name':sb_program['name'],'version':sb_program['version']},'registry_archives':registry,'missing_selected_archives':missing}

def snapshot_registry_cache(evidence:Path)->dict:
    """Retain raw downloaded archives even when metadata/semantic validation fails."""
    cargo_home=Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))
    destination=evidence/'dependency-archives';destination.mkdir(exist_ok=True)
    expected={}
    lock_error=None
    if LOCK.exists():
        try:
            for package in tomllib.loads(LOCK.read_text()).get('package',[]):
                expected[f"{package['name']}-{package['version']}.crate"]=package.get('checksum')
        except Exception as error:lock_error=str(error)
    records=[]
    for archive in sorted((cargo_home/'registry/cache').glob('*/*.crate')):
        actual=sha(archive);target=destination/archive.name
        if target.exists() and sha(target)!=actual:
            target=destination/(actual[:16]+'-'+archive.name)
        if not target.exists():shutil.copyfile(archive,target)
        records.append({'file':target.name,'sha256':actual,'bytes':archive.stat().st_size,'locked_checksum':expected.get(archive.name),'matches_locked_checksum':actual==expected.get(archive.name),'source_cache_namespace':archive.parent.name})
    result={'archives':records,'lock_parse_error':lock_error,'scope':'Raw available downloads preserved even after failure; this snapshot does not itself establish source identity or successful resolution.'}
    (evidence/'downloaded-source-snapshot.json').write_text(json.dumps(result,indent=2)+'\n')
    return {'archive_count':len(records),'snapshot':'downloaded-source-snapshot.json','lock_parse_error':lock_error}

def main()->int:
    parser=argparse.ArgumentParser();parser.add_argument('--evidence-dir',type=Path);parser.add_argument('--validate-only',action='store_true');parser.add_argument('--prepare-only',action='store_true');args=parser.parse_args()
    validate_config()
    if args.validate_only:
        print('standalone manifest, exact primary versions and SDK feature contract valid; no build/resolution performed')
        return 0
    if args.evidence_dir is None:parser.error('--evidence-dir required')
    evidence=args.evidence_dir.resolve();evidence.mkdir(parents=True,exist_ok=True)
    if REPO==evidence or REPO in evidence.parents:
        raise RuntimeError('Use a separate evidence directory outside the repository')
    if args.prepare_only:
        shutil.copytree(PROBE,evidence/'probe-source',dirs_exist_ok=True,ignore=shutil.ignore_patterns('Cargo.lock','target','evidence','__pycache__'))
        head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip()
        with (evidence/'repository-source.tar.gz').open('wb') as out:
            subprocess.run(['git','archive','--format=tar.gz',head],cwd=REPO,stdout=out,check=True)
        (evidence/'prepared.json').write_text(json.dumps({'prepared_utc':now(),'source_commit':head,'status':'toolchain/checks have not run'},indent=2)+'\n')
        return 0
    protected=[REPO/x for x in ['Cargo.lock','Cargo.toml','services/Cargo.lock','pnpm-lock.yaml','programs/shortwin/Cargo.toml'] if (REPO/x).exists()]+[LOCK]
    before={str(x.relative_to(REPO)):sha(x) for x in protected}
    report={'started_utc':now(),'status':'running','steps':[],'valid_attestation_success_tests':0,'signatures_generated':0,'oracle_or_transaction_network_calls':0,'SBF_builds':0,'lock_status':'pinned independently verified initial graph; current-run verification pending','protected_before':before,'platform':platform.platform(),'github':{k:os.environ.get(k) for k in ['GITHUB_SHA','GITHUB_REF','GITHUB_RUN_ID','GITHUB_RUN_ATTEMPT','GITHUB_EVENT_NAME']}}
    exit_code=1
    try:
        source=evidence/'probe-source';shutil.copytree(PROBE,source,dirs_exist_ok=True,ignore=shutil.ignore_patterns('Cargo.lock','target','evidence','__pycache__'))
        for executable,argv,name in [('cargo',['cargo','--version'],'cargo-version.log'),('rustc',['rustc','-vV'],'rustc-version.log')]:
            assert shutil.which(executable),executable
            step=command(argv,evidence/name);report['steps'].append(step)
            if step['exit_code']:raise RuntimeError(name+' failed')
        head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip();report['source_commit']=head
        if os.environ.get('GITHUB_SHA') and head!=os.environ['GITHUB_SHA']:
            raise RuntimeError('checkout HEAD does not match workflow source SHA')
        if subprocess.check_output(['git','status','--porcelain=v1','--untracked-files=no'],cwd=REPO,text=True).strip():
            raise RuntimeError('tracked source must be clean before native probe')
        with (evidence/'repository-source.tar.gz').open('wb') as out:
            subprocess.run(['git','archive','--format=tar.gz',head],cwd=REPO,stdout=out,check=True)
        shutil.copyfile(LOCK,evidence/'Cargo.lock');report['input_lock_sha256']=sha(LOCK);report['resolved_lock_sha256']=sha(LOCK)
        assert report['input_lock_sha256']==EXPECTED['locked_resolution']['lock_sha256']
        step=command(['cargo','metadata','--locked','--format-version','1','--manifest-path',str(MANIFEST)],evidence/'cargo-metadata.json',json_output=True);report['steps'].append(step)
        if step['exit_code']:raise RuntimeError('locked metadata failed')
        metadata=json.loads((evidence/'cargo-metadata.json').read_text())
        report['resolution']=retain_sources(evidence,metadata)
        report['lock_status']='committed standalone lock reused with --locked and current-run source/feature checks; initial independently verified graph preserved'
        for argv,name in [
            (['cargo','tree','--locked','--manifest-path',str(MANIFEST),'-e','features'],'cargo-feature-tree.log'),
            (['cargo','fmt','--manifest-path',str(MANIFEST),'--','--check'],'cargo-fmt.log'),
            (['cargo','test','--locked','--manifest-path',str(MANIFEST),'--all-targets'],'cargo-test.log'),
            (['cargo','clippy','--locked','--manifest-path',str(MANIFEST),'--all-targets','--','-D','warnings'],'cargo-clippy.log')]:
            step=command(argv,evidence/name);report['steps'].append(step)
            # Capture all diagnostics, including clippy after a test/compile failure.
        exit_code=int(any(x['exit_code']for x in report['steps']))
        report['status']='native-checks-passed' if not exit_code else 'native-checks-failed'
    except Exception as error:
        report['status']='failed';report['error']=str(error);print(str(error),file=sys.stderr)
    finally:
        try:
            report['downloaded_source_retention']=snapshot_registry_cache(evidence)
        except Exception as error:
            report['source_retention_error']=str(error);exit_code=1;report['status']='failed'
        if LOCK.exists():shutil.copyfile(LOCK,evidence/'Cargo.lock')
        tracked_after=subprocess.check_output(['git','status','--porcelain=v1','--untracked-files=no'],cwd=REPO,text=True).strip()
        report['tracked_source_changes_after']=tracked_after
        if tracked_after:
            exit_code=1;report['status']='failed'
        after={str(x.relative_to(REPO)):sha(x) for x in protected};report['protected_after']=after
        if after!=before:report['protected_files_changed']=True;exit_code=1;report['status']='failed'
        report['finished_utc']=now()
        report['reproducibility_limit']='This run reuses the independently verified initial standalone lock with --locked and checks its unchanged hash. Source/graph reproducibility is distinct from native ABI success, SBF qualification, oracle attestation and release approval.'
        (evidence/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        (evidence/'SHA256SUMS').write_text(''.join(sha(f)+'  '+str(f.relative_to(evidence))+'\n'for f in sorted(evidence.rglob('*'))if f.is_file()and f.name!='SHA256SUMS'))
    return exit_code
if __name__=='__main__':sys.exit(main())

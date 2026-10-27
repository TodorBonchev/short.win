"""Fault-injection tests. Fake executables never resolve/build/install dependencies."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys,tempfile,unittest

PROBE=Path(__file__).resolve().parent
FAKE_COMMAND=r"""#!/usr/bin/env python3
import hashlib,json,os,pathlib,sys
name=pathlib.Path(sys.argv[0]).name
args=sys.argv[1:]
if name=='git':
    if 'rev-parse'in args:print('a'*40)
    elif 'archive'in args:sys.stdout.buffer.write(b'OFFLINE_FAULT_TEST_NOT_A_REAL_ARCHIVE')
    elif 'status'in args:pass
    else:sys.exit(2)
elif name=='rustc':print('OFFLINE_FAULT_TEST_RUSTC')
elif name=='cargo':
    if '--version'in args:print('OFFLINE_FAULT_TEST_CARGO')
    elif 'generate-lockfile'in args:
        raw=b'OFFLINE_FAULT_TEST_CRATE_BYTES'
        checksum=hashlib.sha256(raw).hexdigest()
        pathlib.Path('Cargo.lock').write_text('version = 4\n[[package]]\nname = "fault-fixture"\nversion = "0.0.0"\nchecksum = "'+checksum+'"\n')
        archive=pathlib.Path(os.environ['CARGO_HOME'])/'registry/cache/fault-test/fault-fixture-0.0.0.crate'
        archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(raw)
    elif 'metadata'in args:
        if os.environ['FAULT_MODE']=='metadata-exit':
            print('OFFLINE_FAULT_TEST_METADATA_FAILURE',file=sys.stderr);sys.exit(7)
        print(json.dumps({'packages':[{'name':'switchboard-on-demand','version':'0.13.0','id':'sdk','source':'registry+OFFLINE_TEST'}],'resolve':{'nodes':[{'id':'sdk','features':['default','solana-v2'],'deps':[]}]}}))
    else:sys.exit(3)
else:sys.exit(4)
"""

class FailureEvidenceTests(unittest.TestCase):
    def check_fault(self,mode):
        with tempfile.TemporaryDirectory(prefix='shortwin-oracle-compat-fault-') as temp:
            base=Path(temp);copy=base/'repo/experiments/jupiter-oracle-compat'
            shutil.copytree(PROBE,copy,ignore=shutil.ignore_patterns('Cargo.lock','target','evidence','__pycache__'))
            tools=base/'tools';tools.mkdir()
            for name in ['git','cargo','rustc']:
                executable=tools/name;executable.write_text(FAKE_COMMAND);executable.chmod(0o755)
            env=os.environ.copy();env.update(PATH=str(tools)+os.pathsep+env.get('PATH',''),CARGO_HOME=str(base/'cargo'),FAULT_MODE=mode)
            env.pop('GITHUB_SHA',None)
            evidence=base/'evidence'
            result=subprocess.run([sys.executable,str(copy/'run_probe.py'),'--evidence-dir',str(evidence)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,check=False)
            self.assertEqual(result.returncode,1,result.stdout+result.stderr)
            report=json.loads((evidence/'report.json').read_text())
            self.assertEqual(report['status'],'failed')
            self.assertEqual(report['valid_attestation_success_tests'],0)
            snapshot=json.loads((evidence/'downloaded-source-snapshot.json').read_text())
            self.assertEqual(len(snapshot['archives']),1)
            archive=evidence/'dependency-archives/fault-fixture-0.0.0.crate'
            self.assertEqual(archive.read_bytes(),b'OFFLINE_FAULT_TEST_CRATE_BYTES')
            self.assertEqual(snapshot['archives'][0]['sha256'],hashlib.sha256(archive.read_bytes()).hexdigest())
            self.assertTrue((evidence/'Cargo.lock').exists())
            self.assertTrue((evidence/'probe-source/.gitignore').exists())
            self.assertIn('dependency-archives/fault-fixture-0.0.0.crate',(evidence/'SHA256SUMS').read_text())
            if mode=='metadata-exit':
                self.assertEqual(report['steps'][-1]['exit_code'],7)
                self.assertIn('METADATA_FAILURE',(evidence/'cargo-metadata.json.stderr.log').read_text())
            else:
                self.assertIn('default',report['error'])
                self.assertTrue((evidence/'cargo-metadata.json').exists())

    def test_metadata_failure_retains_available_archives_and_diagnostics(self):
        self.check_fault('metadata-exit')

    def test_feature_contract_failure_still_retains_downloaded_archives(self):
        self.check_fault('unexpected-features')

if __name__=='__main__':
    suite=unittest.defaultTestLoader.loadTestsFromTestCase(FailureEvidenceTests)
    if suite.countTestCases()!=2:raise RuntimeError('exactly two fault-retention tests must run')
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(0 if result.wasSuccessful() else 1)

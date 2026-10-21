from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,os,subprocess,time
base=Path(__file__).resolve().parent
meta=json.loads((base/'metadata.json').read_text());env=os.environ.copy();env.update(meta['environment'])
start=datetime.now(timezone.utc).isoformat();tick=time.monotonic()
with (base/'run.log').open('wb') as log:
 proc=subprocess.Popen(meta['command'],cwd=meta['cwd'],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT)
 (base/'running.json').write_text(json.dumps({'supervisor_pid':os.getpid(),'runner_pid':proc.pid,'started_utc':start},indent=2)+'\n')
 rc=proc.wait()
result={'started_utc':start,'finished_utc':datetime.now(timezone.utc).isoformat(),'elapsed_seconds':time.monotonic()-tick,'exit_code':rc}
for label,path in [('summary',base/'results/summary.json'),('log',base/'run.log')]:
 if path.exists():result[label+'_sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
(base/'completion.json').write_text(json.dumps(result,indent=2)+'\n')

#!/usr/bin/env python3
"""One existing financial runner, no restart and no combined profiles."""
import datetime,hashlib,json,os,subprocess,sys,time
from pathlib import Path
import verify
ROOT=Path(__file__).resolve().parent

def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def atomic(name,value):
    p=ROOT/name;t=p.with_name(p.name+'.tmp')
    with t.open('w') as f:json.dump(value,f,indent=2,sort_keys=True);f.write('\n');f.flush();os.fsync(f.fileno())
    os.replace(t,p)
    fd=os.open(ROOT,os.O_RDONLY|os.O_DIRECTORY)
    try:os.fsync(fd)
    finally:os.close(fd)
def identity(pid):
    base=Path('/proc')/str(pid);tail=(base/'stat').read_text().rsplit(')',1)[1].split()
    return {'pid':pid,'start_ticks':tail[19],'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'cmdline':[x.decode() for x in (base/'cmdline').read_bytes().split(b'\0') if x],'exe':os.readlink(base/'exe'),'exe_sha256':verify.digest(base/'exe')}
def main():
    need=verify.need;plan=json.loads((ROOT/'launch.json').read_text())
    need(not (ROOT/'supervisor-running.json').exists() and not (ROOT/'outer-completion.json').exists(),'refuse a second launch')
    need(verify.digest(ROOT/'baseline.json')==plan['baseline_sha256'],'fixed baseline anchor')
    for name,h in plan['control_sha256'].items():need(verify.digest(ROOT/name)==h,'control hash '+name)
    need(verify.digest(ROOT/plan['source_archive'])==plan['source_archive_sha256'],'source archive hash')
    b=verify.package(ROOT);command=plan['command'];result=ROOT/'results'
    need(command==[str(ROOT/'package/runner'),'--seed',str(b['seed']),'--duration-seconds','86400','--out',str(result)],'fixed all-day command')
    need(not result.exists(),'refuse existing results')
    env=os.environ.copy();env.update({'SHORTWIN_SIMPLE_SO':str(ROOT/'package/inputs/shortwin_simple.so'),'SHORTWIN_SIMPLE_FIXTURE_ROOT':str(ROOT/'package/inputs'),'SHORTWIN_SIMPLE_SOURCE_ROOT':str(ROOT/'package/source'),'RUST_BACKTRACE':'1'});env.pop('SHORTWIN_JUPITER_CANDIDATE_DIR',None)
    started=time.monotonic();start_utc=utc()
    with (ROOT/'child.stdout.log').open('xb') as output:
        child=subprocess.Popen(command,cwd=ROOT,env=env,stdin=subprocess.DEVNULL,stdout=output,stderr=subprocess.STDOUT)
        actual=identity(child.pid);need(actual['exe_sha256']==b['runner_sha256'] and actual['cmdline']==command,'actual process command/bytes')
        running={'schema':1,'started_at_utc':start_utc,'supervisor_identity':identity(os.getpid()),'actual_child_identity':actual,'command':command,'launch_sha256':verify.digest(ROOT/'launch.json'),'baseline_sha256':plan['baseline_sha256'],'scope':b['scope']}
        atomic('supervisor-running.json',running)
        while child.poll() is None:
            log=result/'run.log'
            atomic('alive.json',{'at_utc':utc(),'elapsed_seconds':time.monotonic()-started,'child_pid':child.pid,'child_start_ticks':actual['start_ticks'],'boot_id':actual['boot_id'],'run_log_bytes':log.stat().st_size if log.exists() else None})
            time.sleep(15)
        outer={'schema':1,'started_at_utc':start_utc,'finished_at_utc':utc(),'exit_code':child.returncode,'elapsed_seconds':time.monotonic()-started,'actual_child_identity':actual,'launch_sha256':running['launch_sha256'],'baseline_sha256':plan['baseline_sha256']}
        try:outer['verification']=verify.verify(ROOT,result,86400,86400,outer)
        except Exception as error:outer['verification']={'verified':False,'error':type(error).__name__+': '+str(error)}
        atomic('outer-completion.json',outer)
        return 0 if outer['verification'].get('verified') else 1
if __name__=='__main__':
    try:sys.exit(main())
    except Exception as error:
        atomic('supervisor-error.json',{'at_utc':utc(),'error':type(error).__name__+': '+str(error)})
        raise

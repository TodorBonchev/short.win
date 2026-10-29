#!/usr/bin/env python3
"""Seeded real-SQLite lifecycle qualification; no network, signing or transactions."""
from __future__ import annotations
import argparse
from collections import Counter
from dataclasses import asdict
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import random
import shutil
import sys
import time

SOURCE=Path(__file__).resolve().parent/"src/shortwin_price_service.py"
SPEC=importlib.util.spec_from_file_location("queue_fuzz_service",SOURCE)
service=importlib.util.module_from_spec(SPEC);sys.modules[SPEC.name]=service;SPEC.loader.exec_module(service)
MARKET="kXYTnbz7MJ3g1d1cKX5wfZLy8KmN7Cctx4oePy5ndB2"
POLICY="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
MINT="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
SETTINGS=service.Settings(60,2,5,30,100,50,1000,1000000000,8,1,1,1)


def require(ok,why):
    if not ok: raise AssertionError("INVARIANT: "+why)
def hash_file(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
def atomic_json(path,value):
    temporary=path.with_name(path.name+'.tmp')
    with temporary.open('wb') as f:
        f.write((json.dumps(value,indent=2,sort_keys=True)+'\n').encode());f.flush();os.fsync(f.fileno())
    os.replace(temporary,path)
    directory=os.open(path.parent,os.O_RDONLY|os.O_DIRECTORY)
    try:os.fsync(directory)
    finally:os.close(directory)
def protected(debt,pending,reference,request,ack,now):
    # Independent declarative model: neither TTL nor a single expired clock can
    # release an identity protected by debt, pending publication or a finite ref.
    if debt:return 'active'
    if pending:return 'pending'
    if reference is not None:
        seconds,slots=reference
        if seconds<=30 or slots<=100:return 'reference'
    if not ack and (request>now or now-request<=270):return 'intent'
    return None


def run_case(seed,out,counts):
    rng=random.Random(seed);base=600+(rng.randrange(10000)*60);slot=base*2+1000
    q=rng.randrange(1,service.U64_MAX+1);item=service.Job(MARKET,POLICY,MINT,q)
    path=out/'working.sqlite3';require(not path.exists(),'prior case database was not disposed')
    h=service.History(path,SETTINGS);counts['database_setups']+=1
    context=slot;now=base;generation=0;retired_evidence={};last_payload=None
    def rows(gen):return h.db.execute("SELECT ts,slot,bid,ask,finite,evidence,reason FROM observations WHERE market=? AND policy=? AND q=? AND generation=? ORDER BY ts",(*item.key,gen)).fetchall()
    def envelope(debt=False,pending=False,ages=None,ack=False,context_override=None):
        refs=[] if ages is None else [{"job":asdict(item),"source_ts":now-ages[0],"source_slot":str(context-ages[1]),"max_age_seconds":30,"max_age_slots":100}]
        return {"schema":1,"complete":True,"commitment":"finalized","publisher_lock":True,"context_slot":str(context if context_override is None else context_override),"clock_slot":str(context),"clock_ts":now,"expected_queue_revision":str(h.revision()),"active_jobs":[asdict(item)] if debt else [],"references":refs,"pending_publications":[asdict(item)] if pending else [],"fulfilled_requests":[{"job":asdict(item),"generation":generation,"requested_at":h.publication_requested_at(item)}] if ack else []}
    def reconcile(debt=False,pending=False,ages=None,ack=False):
        nonlocal context,now,last_payload
        before=rows(generation);request=h.publication_requested_at(item)
        expected=protected(debt,pending,ages,request,ack,now)
        payload=envelope(debt,pending,ages,ack);last_payload=payload
        result=h.reconcile_queue(payload,now);counts['reconciliations']+=1
        require(bool(result['retired'])==(expected is None),'independent protected-set retirement decision')
        if expected:
            counts['protected_'+expected]+=1;require(h.generation(item)==generation,'protected identity replaced')
            if expected=='reference' and ages[0]>30 and ages[1]<=100:counts['protected_reference_time_expired_only']+=1
            if expected=='reference' and ages[1]>100 and ages[0]<=30:counts['protected_reference_slots_expired_only']+=1
        else:
            counts['retirements']+=1;retired_evidence[generation]=before
            require(h.generation(item) is None,'retired identity still scheduled')
        for gen,evidence in retired_evidence.items():require(rows(gen)==evidence,'retired raw observations/evidence changed')
        counts['evidence_checks']+=len(retired_evidence)
        context+=1;now+=1
        return result
    def readmit():
        nonlocal generation
        require(h.enqueue(item,now),'readmission refused despite available slot');counts['admissions']+=1
        generation+=1;require(h.generation(item)==generation,'readmission generation did not advance')
    try:
        require(h.enqueue(item,base-400),'initial admission');counts['admissions']+=1
        for timestamp in range(base-120,base+1,5):
            h.record(item,timestamp,slot-(base-timestamp),100,105,{"case_seed":seed,"timestamp":timestamp});counts['observations']+=1
        require(h.reference(item,0,slot,base)['status']=='Finite','constructed old generation history is not finite')
        h.enqueue(item,now);counts['warm_requests']+=1
        h.db.execute("INSERT INTO metadata VALUES('financial_allowance_sentinel','4:1000000')");h.db.commit()
        reconcile() # fresh unfulfilled intent
        before=h.queue_state();replay=h.reconcile_queue(last_payload,last_payload['clock_ts'])
        require(replay['idempotent'] and h.queue_state()==before,'unchanged exact snapshot retry failed');counts['idempotent_replays']+=1
        alternative=service.Job(MARKET,POLICY,MINT,q-1 if q>1 else q+1,True)
        require(not h.enqueue(alternative,now),'full warm protected queue evicted an identity');counts['full_queue_refusals']+=1
        reconcile(debt=True)
        reconcile(pending=True,ack=True)
        reconcile(ages=(0,0),ack=True)
        reconcile(ages=(30,100),ack=True)
        context+=200;reconcile(ages=(2,197),ack=True) # slots expired, time fresh
        now+=32;reconcile(ages=(31,50),ack=True) # time expired, slots fresh
        context+=101;reconcile(ages=(32,101),ack=True)
        readmit();h.record(item,now,context,100,105,expected_generation=generation);counts['observations']+=1
        require(h.reference(item,0,context,now)['status']=='Unknown','cold cohort reused completed old history');counts['cold_history_checks']+=1
        try:h.record(item,now+1,context+1,100,105,expected_generation=generation-1)
        except ValueError:counts['stale_tick_rejections']+=1
        else:require(False,'stale tick wrote across generation')
        stale=envelope(ack=True);other=service.History(path,SETTINGS)
        try:other.enqueue(item,now)
        finally:other.close()
        before=h.queue_state()
        try:h.reconcile_queue(stale,now)
        except ValueError:counts['cas_rejections']+=1
        else:require(False,'stale revision retired a raced request')
        require(h.queue_state()==before,'failed CAS mutated scheduling')
        for patch in [{"complete":False},{"publisher_lock":False},{"commitment":"confirmed"},{"context_slot":"1"}]:
            bad=envelope();bad.update(patch);before=h.queue_state()
            try:h.reconcile_queue(bad,now)
            except ValueError:counts['snapshot_rejections']+=1
            else:require(False,'invalid trusted snapshot was accepted')
            require(h.queue_state()==before,'rejected snapshot mutated queue')
        h.close();h=service.History(path,SETTINGS);counts['restarts']+=1
        require(h.generation(item)==generation,'restart lost admission generation')
        before=h.queue_state()
        try:h.reconcile_queue(last_payload,last_payload['clock_ts'])
        except ValueError:counts['cached_replay_rejections']+=1
        else:require(False,'cached snapshot replay accepted changed admission state')
        require(h.queue_state()==before,'rejected cached snapshot changed readmitted state')
        for gen,evidence in retired_evidence.items():require(rows(gen)==evidence,'restart lost retired evidence')
        for _ in range(rng.randrange(8,25)):
            context+=rng.randrange(1,150);now+=rng.randrange(1,40)
            debt=rng.choice([True,False]);pending=rng.randrange(4)==0;ack=rng.choice([True,False])
            ages=rng.choice([None,(0,0),(31,50),(20,150),(40,150)])
            result=reconcile(debt,pending,ages,ack)
            if result['retired']:readmit()
            else:
                alternative=service.Job(MARKET,POLICY,MINT,q-1 if q>1 else q+1,True)
                require(not h.enqueue(alternative,now),'full protected queue evicted an identity');counts['full_queue_refusals']+=1
        require(h.db.execute("SELECT value FROM metadata WHERE key='financial_allowance_sentinel'").fetchone()[0]=='4:1000000','capacity reuse reset financial allowance')
        counts['financial_allowance_checks']+=1
    except BaseException:
        # Capture raw recovery state before SQLite.close can checkpoint or remove
        # its WAL/SHM files. The replay seed remains in the main failure record.
        crash=out/'failure-sqlite';crash.mkdir(exist_ok=False)
        preserved={}
        for suffix in ['', '-wal', '-shm']:
            original=Path(str(path)+suffix)
            if original.exists():
                copy=crash/original.name;shutil.copyfile(original,copy)
                with copy.open('rb') as copied:os.fsync(copied.fileno())
                preserved[copy.name]=hash_file(copy)
        atomic_json(crash/'SHA256SUMS.json',preserved)
        raise
    finally:h.close()
    for suffix in ['', '-wal', '-shm']:
        file=Path(str(path)+suffix)
        if file.exists():file.unlink()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seed',type=int,default=103410000)
    mode=parser.add_mutually_exclusive_group()
    mode.add_argument('--cases',type=int)
    mode.add_argument('--duration-seconds',type=int)
    mode.add_argument('--replay-seed',type=int)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--stop-file',type=Path)
    args=parser.parse_args()
    service.integer(args.seed,'seed',0,service.U64_MAX)
    if args.cases is not None and args.cases<=0:parser.error('--cases must be positive')
    if args.duration_seconds is not None and args.duration_seconds<=0:parser.error('--duration-seconds must be positive')
    if args.replay_seed is not None:service.integer(args.replay_seed,'replay seed',0,service.U64_MAX)
    args.out.mkdir(parents=True,exist_ok=False)
    case_limit=1 if args.replay_seed is not None else args.cases if args.cases is not None else None if args.duration_seconds is not None else 64
    metadata={'schema':1,'seed':args.seed,'case_limit':case_limit,'requested_duration_seconds':args.duration_seconds,
              'replay_seed':args.replay_seed,'command':sys.argv,'python_version':sys.version,
              'source_sha256':{'queue_lifecycle_fuzz.py':hash_file(Path(__file__).resolve()),'src/shortwin_price_service.py':hash_file(SOURCE)},
              'model_hash':SETTINGS.model_hash,'synthetic_settings':asdict(SETTINGS),
              'scope':'real SQLite lifecycle operations with independent protected-set model; no RPC, price-provider throughput, signatures, funded operation or on-chain execution'}
    atomic_json(args.out/'metadata.json',metadata)
    log=(args.out/'run.log').open('xb');counts=Counter();completed=0;attempted=0;error=None;stopped=False;invariant_failures=0;setup_failures=0
    rng=random.Random(args.seed);started=time.monotonic()
    def event(value):
        log.write((json.dumps(value,sort_keys=True)+'\n').encode());log.flush()
    try:
        while case_limit is None or completed<case_limit:
            if args.stop_file is not None and args.stop_file.exists():
                stopped=True;event({'event':'supervisor_stop','completed_cases':completed});break
            if args.duration_seconds is not None and time.monotonic()-started>=args.duration_seconds:break
            case_seed=args.replay_seed if args.replay_seed is not None else rng.getrandbits(64)
            event({'event':'case_start','index':attempted,'seed':case_seed});attempted+=1
            before_setup=counts['database_setups']
            try:run_case(case_seed,args.out,counts)
            except BaseException as failure:
                error={'case_index':attempted-1,'case_seed':case_seed,'error':repr(failure)}
                invariant_failures=int(isinstance(failure,AssertionError));setup_failures=int(counts['database_setups']==before_setup)
                atomic_json(args.out/'failure.json',error);event({'event':'failure',**error});break
            completed+=1
            event({'event':'case_complete','completed_cases':completed,'wall_seconds':time.monotonic()-started})
        for name,expected in metadata['source_sha256'].items():
            source=Path(__file__).resolve().parent/name
            if hash_file(source)!=expected:
                error={'stage':'source_identity','error':'worker source changed during run','source':name}
                invariant_failures=1;atomic_json(args.out/'failure.json',error);event({'event':'failure',**error});break
    finally:
        log.flush();os.fsync(log.fileno());log.close()
    elapsed=time.monotonic()-started
    clean=error is None and not stopped and completed>0 and (args.duration_seconds is None or elapsed>=args.duration_seconds)
    code=0 if clean else 3 if stopped else 1
    summary={'schema_version':1,'completed_cases':completed,'attempted_cases':attempted,'wall_seconds':elapsed,
             'requested_duration_seconds':args.duration_seconds,'failures':int(error is not None),'anomalies':int(error is not None),
             'setup_failures':setup_failures,'invariant_failures':invariant_failures,'finished_cleanly':clean,'stopped':stopped,
             'coverage':dict(counts),'scope':metadata['scope']}
    atomic_json(args.out/'summary.json',summary)
    if stopped:atomic_json(args.out/'stopped.json',{'reason':'supervisor stop file observed between cases','completed_cases':completed,'wall_seconds':elapsed})
    atomic_json(args.out/'completion.json',{'exit_code':code,'elapsed_seconds':elapsed,
                'summary_sha256':hash_file(args.out/'summary.json'),'log_sha256':hash_file(args.out/'run.log')})
    print(json.dumps({'status':'passed' if clean else 'stopped' if stopped else 'failed','completed_cases':completed,'wall_seconds':elapsed,'output':str(args.out)}))
    return code


if __name__=='__main__':raise SystemExit(main())

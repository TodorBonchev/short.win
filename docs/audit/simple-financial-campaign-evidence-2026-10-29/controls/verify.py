#!/usr/bin/env python3
"""Read-only verifier for the existing single financial runner; no dispatch."""
import hashlib,json,math
from pathlib import Path

def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def need(ok,why):
    if not ok:raise ValueError(why)
def package(root):
    root=Path(root);b=json.loads((root/'baseline.json').read_text());p=root/'package'
    need(digest(p/'runner')==b['runner_sha256'],'package runner hash')
    for name,h in b['source_hashes'].items():need(digest(p/'source'/name)==h,'package source '+name)
    for name,h in b['artifact_hashes'].items():need(digest(p/'inputs'/name)==h,'package input '+name)
    return b

def verify(root,result,minimum,expected_duration,outer=None):
    root=Path(root);result=Path(result)
    if minimum>=86400:need(outer is not None,'final verification requires actual supervisor completion')
    if outer is not None and minimum>=86400:
        need(digest(root/'baseline.json')==outer['baseline_sha256'],'fixed startup baseline anchor')
        need(digest(root/'launch.json')==outer['launch_sha256'],'fixed startup launch anchor')
        plan=json.loads((root/'launch.json').read_text())
        need(plan['baseline_sha256']==outer['baseline_sha256'],'launch baseline binding')
        for name,h in plan['control_sha256'].items():need(digest(root/name)==h,'final control hash '+name)
        need(digest(root/plan['source_archive'])==plan['source_archive_sha256'],'final source archive hash')
    b=package(root)
    m=json.loads((result/'metadata.json').read_text());s=json.loads((result/'summary.json').read_text());c=json.loads((result/'completion.json').read_text())
    need(m['seed']==b['seed'] and m['case_limit'] is None,'seed/duration-mode case limit')
    need(m['requested_duration_seconds']==expected_duration,'metadata duration')
    need(m['command']==[str(root/'package/runner'),'--seed',str(b['seed']),'--duration-seconds',str(expected_duration),'--out',str(result)],'actual command')
    need(m['frozen']['runner_sha256']==b['runner_sha256'] and digest(result/'runner')==b['runner_sha256'],'frozen runner hash')
    need(m['frozen']['source_hashes']==b['source_hashes'],'source manifest drift')
    need(m['frozen']['artifact_hashes']==b['artifact_hashes'],'input manifest drift')
    for name,h in b['source_hashes'].items():need(digest(result/'source'/name)==h,'frozen source '+name)
    for name,h in b['artifact_hashes'].items():need(digest(result/'frozen-root'/name)==h,'frozen input '+name)
    need(c['exit_code']==0 and s['finished_cleanly'] is True,'runner exit/clean flag')
    for k in ['failures','anomalies','setup_failures','invariant_failures']:need(type(s[k]) is int and s[k]==0,k)
    wall=s['wall_seconds'];need(type(wall) in [int,float] and math.isfinite(wall) and wall>=minimum,'insufficient runner duration')
    need(c['elapsed_seconds']==wall and s['requested_duration_seconds']==expected_duration,'completion/summary duration')
    need(c['summary_sha256']==digest(result/'summary.json') and c['log_sha256']==digest(result/'run.log'),'summary/log hashes')
    need(not list(result.glob('crash*')) and not list(result.glob('anomaly*')),'crash/anomaly artifact')
    coverage=s['coverage'];scenarios=coverage['scenario_counts'];need(set(scenarios)==set(b['scenario_names']) and all(type(v) is int and v>0 for v in scenarios.values()),'scenario coverage')
    need(sum(scenarios.values())==s['completed_cases'],'scenario total')
    need(coverage['successful_fixture_setups']==coverage['attempted_fixture_setups']==s['completed_cases'],'fixture setup totals')
    need(coverage['attempted_transactions']==coverage['successful_transactions']+coverage['expected_rejected_transactions'],'driver transaction totals')
    need(coverage['actual_cpmm_success_logs']>0,'no actual CPMM success logs')
    if outer is not None:
        need(outer['exit_code']==0 and outer['elapsed_seconds']>=minimum and outer['elapsed_seconds']>=wall,'supervisor exit/duration')
        need(outer['actual_child_identity']['exe_sha256']==b['runner_sha256'],'actual process executable')
    return {'verified':True,'scope':b['scope'],'wall_seconds':wall,'completed_cases':s['completed_cases'],'coverage':coverage,'runner_sha256':b['runner_sha256'],'source_files':len(b['source_hashes']),'input_files':len(b['artifact_hashes']),'metadata_sha256':digest(result/'metadata.json'),'summary_sha256':digest(result/'summary.json'),'log_sha256':digest(result/'run.log'),'completion_sha256':digest(result/'completion.json')}

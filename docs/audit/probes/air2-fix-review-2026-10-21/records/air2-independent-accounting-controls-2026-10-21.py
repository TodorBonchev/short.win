from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
repo=Path("/root/short.win-audit-review")
out=repo/"target/air2-independent-accounting-controls-2026-10-21"
out.mkdir(parents=True,exist_ok=True)
env=os.environ.copy()
env["PATH"]="/root/.cargo/bin:/root/.local/share/solana/install/active_release/bin:/usr/local/bin:/usr/bin:/bin"
env["CARGO_TARGET_DIR"]="/root/short.win/target"
env["CARGO_BUILD_JOBS"]="2"
env["RUST_TEST_THREADS"]="2"
results=[]
def run(label,args,cwd=repo,expect=0):
    p=subprocess.run(args,cwd=cwd,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    log="$ "+" ".join(args)+"\n"+p.stdout+"\nEXIT "+str(p.returncode)+"\n"
    (out/(label+".log")).write_text(log)
    results.append({"label":label,"command":args,"cwd":str(cwd),"exit":p.returncode,"expected":expect,"log_sha256":hashlib.sha256(log.encode()).hexdigest()})
    print(label+": exit="+str(p.returncode)+" expected="+str(expect),flush=True)
    print("\n".join(x for x in p.stdout.splitlines() if x.startswith("test ") or "panicked" in x or "left:" in x or "right:" in x or "PRE_FIX" in x),flush=True)
    if p.returncode!=expect:
        print(p.stdout[-5000:],flush=True)
        raise RuntimeError(label)
base=["cargo","test","--locked","-p","shortwin-math"]
run("candidate-air2-01",base+["--test","ai2_accounting","air2_01_a_fully_locked_lender_stays_locked_when_conversion_awards_a_unit","--","--exact","--nocapture"])
run("candidate-l39",base+["--test","ai2_accounting","l39_one_token_deposit_cannot_unlock_a_fully_locked_position","--","--exact","--nocapture"])
run("candidate-conversion-units",base+["--lib","pool::tests::convert","--","--nocapture"])
for test in ["idle_lenders_expired_lock_stops_counting_at_lock_until","stuck_lock_clears_at_lock_until","lock_decay_restarts_with_every_lock_era"]:
    run("candidate-"+test,base+["--test","pool_econ",test,"--","--exact","--nocapture"])
math=out/"math"
controls=out/"controls"
(math/"src").mkdir(parents=True,exist_ok=True)
(controls/"tests").mkdir(parents=True,exist_ok=True)
shutil.copytree(repo/"crates/shortwin-math/src",math/"src",dirs_exist_ok=True,copy_function=shutil.copyfile)
(math/"Cargo.toml").write_text('[package]\nname="shortwin-math"\nversion="0.1.0"\nedition="2024"\n[workspace]\n')
(controls/"Cargo.toml").write_text('[package]\nname="air2-independent-controls"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\nshortwin-math={path="../math"}\n')
for name in ["ai2_accounting.rs","pool_harness.rs"]:
    shutil.copyfile(repo/"crates/shortwin-math/tests"/name,controls/"tests"/name)
edges=r'''
use shortwin_math::pool::{self,Lender,Pool,ShareEpoch,Standing,Vault};
use shortwin_math::num::WAD;
fn convert(shares:u128,lock:u128,k:u8,rem:u128,wipe:bool)->Lender {
    let mut p=Pool::new();p.share_epoch=1;
    let den=1000u128.pow(u32::from(k));
    let old_total=shares.checked_add(rem).unwrap();
    p.total_shares=if wipe {0} else {old_total/den};
    let mut l=Lender{shares,lock,lock_term:lock,lock_pi:WAD,lock_until:86400,epoch:0,..Lender::new(&p)};
    let mut r=[ShareEpoch{acc_end:0,acc_r_end:0,s_old:old_total,carry:0,carry_r:0,conv_rem:rem,epoch:0,num:if wipe{0}else{1},den_log1000:k,_pad:[0;10]}];
    assert_eq!(pool::convert(&p,&mut l,&mut r).unwrap(),Standing::Current);
    l
}
#[test]
fn fully_locked_conversion_keeps_every_awarded_share_locked() {
    let mut cases=0;
    for k in [0u8,1,2,12] {
        let den=1000u128.pow(u32::from(k));
        for shares in [0,1,den-1,den,den+1,2*den-1,u128::MAX] {
            for rem in [0,den-1] {
                if shares.checked_add(rem).is_none(){continue;}
                let l=convert(shares,shares,k,rem,false);
                assert_eq!(l.lock,l.shares,"old shares={shares} denominator={den} remainder={rem}");
                cases+=1;
            }
        }
    }
    println!("FULL_LOCK_CASES={cases}");
}
#[test]
fn zero_lock_stays_zero_even_when_cumulative_rounding_awards_a_share() {
    let l=convert(2_500_999,0,1,1,false);
    assert_eq!((l.shares,l.lock),(2501,0));
    let tiny=convert(1,0,1,999,false);
    assert_eq!((tiny.shares,tiny.lock),(1,0));
}
#[test]
fn partial_lock_rounding_is_conservative_and_full_wipe_clears_lock() {
    let l=convert(2_500_999,1_000_001,1,1,false);
    assert_eq!((l.shares,l.lock,l.lock_term),(2501,1001,1000));
    let without_award=convert(2_500_999,1_000_001,1,0,false);
    assert_eq!((without_award.shares,without_award.lock),(2500,1001));
    let tiny=convert(1,1,1,0,false);
    assert_eq!((tiny.shares,tiny.lock),(0,0));
    let wiped=convert(u128::MAX,u128::MAX,0,0,true);
    assert_eq!((wiped.shares,wiped.lock,wiped.lock_term),(0,0,0));
}
#[test]
fn allowance_only_exposes_new_unlocked_value_and_expiry_releases_old_value() {
    let mut p=Pool::new();p.total_shares=2_000_000;p.tokens_free=1_000;
    let mut v=Vault::new();v.lock_norm=p.total_shares;v.lock_cal[1]=v.lock_norm;v.cal_day=1;
    let mut l=Lender{shares:1_000_000,lock:1_000_000,lock_term:1_000_000,lock_pi:WAD,lock_epoch:v.pi_epoch,lock_until:86400,..Lender::new(&p)};
    assert_eq!(pool::allowance(&p,&v,&l,1),Ok(0));
    l.shares+=10_000;p.total_shares+=10_000;p.tokens_free+=5;
    assert_eq!(pool::allowance(&p,&v,&l,1),Ok(5));
    assert!(pool::allowance(&p,&v,&l,86400).unwrap()>5);
    assert_eq!(pool::allowance_view(&p,&v,&l,&[],1),pool::allowance(&p,&v,&l,1));
}
'''
(controls/"tests/reviewer_edges.rs").write_text(edges)
control=["cargo","test","--offline","--manifest-path",str(controls/"Cargo.toml")]
run("candidate-reviewer-edges",control+["--test","reviewer_edges","--","--nocapture"])
pool=math/"src/pool.rs"
current=pool.read_text()
oldlock='''        l.lock = if num == 0 {
            0
        } else {
            l.lock.div_ceil(den).min(shares)
        };'''
assert current.count(oldlock)==1
pool.write_text(current.replace(oldlock,"        l.lock = scale(l.lock)?;"))
run("floor-lock-strengthened-regression-negative-control",control+["--test","ai2_accounting","air2_01_a_fully_locked_lender_stays_locked_when_conversion_awards_a_unit","--","--exact","--nocapture"],expect=101)
run("floor-lock-reviewer-negative-control",control+["--test","reviewer_edges","fully_locked_conversion_keeps_every_awarded_share_locked","--","--exact","--nocapture"],expect=101)
assert current.count("let by_value = tokens_for_shares(pool, u)?;")==1
pool.write_text(current.replace("let by_value = tokens_for_shares(pool, u)?;","let by_value = tokens_for_shares(pool, lender.shares)?;"))
run("old-value-cap-negative-control",control+["--test","ai2_accounting","l39_one_token_deposit_cannot_unlock_a_fully_locked_position","--","--exact","--nocapture"],expect=101)
pre_fix=r'''
#[path="pool_harness.rs"]mod pool_harness;
use pool_harness::Model;
use shortwin_math::pool;
fn exit_allowance(s:&mut Model,who:usize){for _ in 0..8{let l=s.touch(who);let p=s.m.pool;let a=s.free_allowance(who).unwrap();let claims=u128::from(p.tokens_free)+u128::from(p.borrowed_q)+pool::VIRTUAL_ASSETS;let z=(p.total_shares+pool::VIRTUAL_SHARES).div_ceil(claims)-1;let burn=l.shares.min(s.shares_at(a).max(z).max(1));if s.withdraw(who,burn).is_err(){break;}}}
#[test]fn old_cap_replays_the_actual_loss_shift(){
let mut s=Model::new(9000);for who in 0..3{s.deposit(who,1_000_000).unwrap();}
s.borrow30(10_000);let sh=s.shares_at(1000);assert_eq!(s.withdraw(2,sh),Ok(1000));
let repaid=s.borrow30(1_000_000);let lost=s.borrow30(1_000_000);for who in 0..3{exit_allowance(&mut s,who);}
s.repay(repaid,0,0).unwrap();let mut control=s.clone();assert_eq!(s.deposit(0,1),Ok(999));
let all=s.shares(0);assert_eq!(s.withdraw(0,all),Ok(670_224));s.settle(lost,0,0).unwrap();control.settle(lost,0,0).unwrap();
let baseline=pool::tokens_for_shares(&control.m.pool,control.shares(0)).unwrap();assert_eq!(baseline,336_779);
println!("PRE_FIX_NET_TOKENS=670223 CONTROL_TOKENS={baseline}");}
'''
(controls/"tests/pre_fix_loss.rs").write_text(pre_fix)
run("old-value-cap-actual-loss-replay",control+["--test","pre_fix_loss","--","--nocapture"])
pool.write_text(current)
# Keep the retained copy equal to the reviewed source; variant evidence is in commands/logs.
hashes={str(p.relative_to(repo)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [repo/"crates/shortwin-math/src/pool.rs",repo/"crates/shortwin-math/tests/ai2_accounting.rs",repo/"crates/shortwin-math/tests/pool_harness.rs"]}
evidence={"head":subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip(),"source_sha256":hashes,"checks":results,"reviewer_edges_sha256":hashlib.sha256(edges.encode()).hexdigest(),"runner_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
(out/"checks.json").write_text(json.dumps(evidence,indent=2)+"\n")
print("ALL_FOCUSED_CHECKS_MATCH_EXPECTATIONS",flush=True)

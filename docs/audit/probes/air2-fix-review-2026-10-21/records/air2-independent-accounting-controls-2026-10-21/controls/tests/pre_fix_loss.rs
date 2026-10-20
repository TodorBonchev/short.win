
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

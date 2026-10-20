
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

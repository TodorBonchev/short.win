use shortwin_math::pool::{self, Lender, Pool, SweepSource, Vault};

const START: i64 = 1_800_000_000;
const VEST: u32 = 21_600;
const INTERMEDIATE: i64 = 7_200;
const LUMP: u64 = 10_800;

#[derive(Clone)]
struct State { p: Pool, v: Vault, a: Lender, b: Lender }

fn origin() -> State {
    let mut p = Pool::new();
    let mut v = Vault::new();
    let mut a = Lender::new(&p);
    pool::accrue(&mut p, &mut v, START, 0).unwrap();
    let minted_a = pool::deposit(&mut p, &mut v, &mut a, &mut [], START, 1, false).unwrap();
    let mut b = Lender::new(&p);
    let minted_b = pool::deposit(&mut p, &mut v, &mut b, &mut [], START, 1, false).unwrap();
    assert_eq!((minted_a, minted_b), (1_000, 1_000));
    assert_eq!(pool::sweep_destination(&p, SweepSource::Trader), pool::SweepDestination::Vest);
    pool::sweep(&mut p, &mut v, None, START, VEST, LUMP, SweepSource::Trader).unwrap();
    assert_eq!((p.vest_bal, p.vest_end, v.lender_sol), (LUMP, START + i64::from(VEST), LUMP));
    State { p, v, a, b }
}

fn scheduled(mut s: State, frequent: bool) -> State {
    if frequent {
        for elapsed in 1..=INTERMEDIATE {
            pool::accrue(&mut s.p, &mut s.v, START + elapsed, 0).unwrap();
        }
    } else {
        pool::accrue(&mut s.p, &mut s.v, START + INTERMEDIATE, 0).unwrap();
    }
    let before = s.p;
    pool::accrue(&mut s.p, &mut s.v, START + INTERMEDIATE, 0).unwrap();
    assert_eq!(s.p, before, "same-time accrual must be idempotent");
    s
}

fn exit_a(mut s: State) -> (State, u64) {
    let shares = s.a.shares;
    let tokens = pool::withdraw(&mut s.p, &mut s.v, &mut s.a, &mut [], START + INTERMEDIATE, 86_400, shares).unwrap();
    assert_eq!(tokens, 1);
    assert_eq!(s.p.total_shares, s.b.shares, "A must not be the last holder");
    let paid = pool::claim(&s.p, &mut s.v, &mut s.a, &mut [], START + INTERMEDIATE).unwrap();
    (s, paid)
}

fn main() {
    let initial = origin();
    let sparse = scheduled(initial.clone(), false);
    let frequent = scheduled(initial, true);
    println!("NATIVE_PRODUCTION_POOL vest_s={} lump={} intermediate_s={} extra_accrual_calls={}", VEST, LUMP, INTERMEDIATE, INTERMEDIATE - 1);
    for (label, original) in [("sparse", sparse), ("frequent", frequent)] {
        println!("{} intermediate vest_bal={} vest_last={} vest_end={} acc={} acc_r={} lender_sol={}", label, original.p.vest_bal, original.p.vest_last, original.p.vest_end, original.p.acc, original.p.acc_r, original.v.lender_sol);
        let mut stayed = original.clone();
        pool::accrue(&mut stayed.p, &mut stayed.v, START + i64::from(VEST), 0).unwrap();
        let stay_a = pool::claim(&stayed.p, &mut stayed.v, &mut stayed.a, &mut [], START + i64::from(VEST)).unwrap();
        let stay_b = pool::claim(&stayed.p, &mut stayed.v, &mut stayed.b, &mut [], START + i64::from(VEST)).unwrap();
        assert_eq!(stayed.p.vest_bal, 0);
        assert_eq!(stay_a + stay_b + stayed.v.lender_sol, LUMP);
        println!("{} unchanged_holders_end paid_a={} paid_b={} vault_remaining={} vest_bal={}", label, stay_a, stay_b, stayed.v.lender_sol, stayed.p.vest_bal);
        let (exited, paid_a) = exit_a(original);
        let mut at_end = exited.clone();
        pool::accrue(&mut at_end.p, &mut at_end.v, START + i64::from(VEST), 0).unwrap();
        let paid_b = pool::claim(&at_end.p, &mut at_end.v, &mut at_end.b, &mut [], START + i64::from(VEST)).unwrap();
        assert_eq!(paid_a + paid_b + at_end.v.lender_sol, LUMP);
        println!("{} a_exits_intermediate paid_a={} b_at_end={} vault_remaining={} vest_bal={}", label, paid_a, paid_b, at_end.v.lender_sol, at_end.p.vest_bal);
        let mut last = exited;
        let b_shares = last.b.shares;
        pool::withdraw(&mut last.p, &mut last.v, &mut last.b, &mut [], START + INTERMEDIATE, 86_400, b_shares).unwrap();
        let last_paid = pool::claim(&last.p, &mut last.v, &mut last.b, &mut [], START + INTERMEDIATE).unwrap();
        assert_eq!((last.p.total_shares, last.p.vest_bal), (0, 0));
        assert_eq!(paid_a + last_paid + last.v.lender_sol, LUMP);
        println!("{} last_holder_flush paid_a={} paid_b={} vault_remaining={} vest_bal={} shares={}", label, paid_a, last_paid, last.v.lender_sol, last.p.vest_bal, last.p.total_shares);
    }
}

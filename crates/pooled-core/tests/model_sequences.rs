//! Deterministic stateful model for token-principal and actually paid SOL.
//! Host-only: this is not an SBF, route, oracle, concurrency or 24h qualification.
use shortwin_pooled_core::*;
#[derive(Clone)]
struct Generation {
    pool: Pool,
    lenders: [Lender; 4],
    loans: Vec<u64>,
}
impl Generation {
    fn new(n: u64) -> Self {
        Self {
            pool: Pool::new(n),
            lenders: [Lender::new(n); 4],
            loans: vec![],
        }
    }
}
#[derive(Default, Debug)]
pub struct Report {
    pub seed: u64,
    pub steps: u64,
    pub successes: u64,
    pub rejected: u64,
    pub generations: u64,
    pub deposits: u128,
    pub donations: u128,
    pub withdrawn_tokens: u128,
    pub defaulted_tokens: u128,
    pub paid_sol: u128,
    pub claimed_sol: u128,
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut n = self.0;
        n ^= n << 13;
        n ^= n >> 7;
        n ^= n << 17;
        self.0 = n;
        n
    }
}
fn invariants(gens: &[Generation], r: &Report) {
    let (mut assets, mut reserve) = (0u128, 0u128);
    for g in gens {
        g.pool.validate().unwrap();
        assert_eq!(
            g.lenders.iter().map(|l| l.shares).sum::<u128>(),
            g.pool.shares
        );
        assert_eq!(g.loans.iter().copied().sum::<u64>(), g.pool.principal_out);
        assert_eq!(g.loans.len() as u32, g.pool.loan_count);
        for l in &g.lenders {
            l.validate(&g.pool).unwrap();
        }
        assets += u128::from(g.pool.principal_assets().unwrap());
        reserve += u128::from(g.pool.sol_reserve);
    }
    assert_eq!(
        assets + r.withdrawn_tokens + r.defaulted_tokens,
        r.deposits + r.donations,
        "token principal conservation"
    );
    assert_eq!(
        reserve + r.claimed_sol,
        r.paid_sol,
        "only actually paid SOL is claimable"
    );
}
fn finish(g: &mut Generation, r: &mut Report) {
    for i in 0..4 {
        if g.lenders[i].shares > 0 {
            let n = g.lenders[i].shares;
            let w = withdraw(&mut g.pool, &mut g.lenders[i], n).unwrap();
            r.withdrawn_tokens += u128::from(w.tokens);
            r.claimed_sol += u128::from(w.sol);
        }
    }
    assert_eq!(
        (
            g.pool.shares,
            g.pool.sol_reserve,
            g.pool.principal_out,
            g.pool.cash
        ),
        (0, 0, 0, 0)
    );
}
pub fn run_model(seed: u64, steps: u64) -> Report {
    let mut rng = Rng(if seed == 0 { 0x9e3779b97f4a7c15 } else { seed });
    let mut gens = vec![Generation::new(0)];
    let mut r = Report {
        seed,
        steps,
        generations: 1,
        ..Report::default()
    };
    for _ in 0..steps {
        let action = rng.next() % 10;
        let active = gens.len() - 1;
        let i = if rng.next().is_multiple_of(4) {
            (rng.next() as usize) % gens.len()
        } else {
            active
        };
        let l = (rng.next() as usize) % 4;
        let amount = rng.next() % 1000;
        let before = gens[i].clone();
        let g = &mut gens[i];
        let result: Result<()> = match action {
            0 | 1 => deposit(&mut g.pool, &mut g.lenders[l], amount).map(|_| {
                r.deposits += u128::from(amount);
            }),
            2 => {
                let n = if g.lenders[l].shares == 0 {
                    0
                } else {
                    u128::from(rng.next()) % (g.lenders[l].shares + 2)
                };
                withdraw(&mut g.pool, &mut g.lenders[l], n).map(|w| {
                    r.withdrawn_tokens += u128::from(w.tokens);
                    r.claimed_sol += u128::from(w.sol);
                })
            }
            3 => claim(&mut g.pool, &mut g.lenders[l]).map(|s| {
                r.claimed_sol += u128::from(s);
            }),
            4 => borrow(&mut g.pool, amount).map(|_| g.loans.push(amount)),
            5 => {
                let q = g.loans.last().copied().unwrap_or(1);
                repay(&mut g.pool, q).map(|_| {
                    g.loans.pop();
                })
            }
            6 => {
                let q = g.loans.last().copied().unwrap_or(1);
                default(&mut g.pool, q, amount).map(|_| {
                    g.loans.pop();
                    r.defaulted_tokens += u128::from(q);
                    r.paid_sol += u128::from(amount);
                })
            }
            7 => credit_income(&mut g.pool, amount).map(|_| {
                r.paid_sol += u128::from(amount);
            }),
            8 => donate_tokens(&mut g.pool, amount).map(|_| {
                r.donations += u128::from(amount);
            }),
            _ => Ok(()),
        };
        match result {
            Ok(()) => r.successes += 1,
            Err(_) => {
                r.rejected += 1;
                assert_eq!(g.pool, before.pool, "rejected pool mutation");
                assert_eq!(g.lenders, before.lenders, "rejected receipt mutation");
                assert_eq!(g.loans, before.loans);
            }
        }
        // Periodically force a complete loss, preserving old SOL claims and moving only
        // newly deposited principal into a new generation. Retain up to16 live ledgers.
        if action == 9 && gens[active].pool.shares > 0 {
            let g = &mut gens[active];
            let cash = g.pool.cash;
            if cash > 0 {
                borrow(&mut g.pool, cash).unwrap();
                g.loans.push(cash);
            }
            while let Some(q) = g.loans.pop() {
                let sol = rng.next() % 1000;
                default(&mut g.pool, q, sol).unwrap();
                r.defaulted_tokens += u128::from(q);
                r.paid_sol += u128::from(sol);
            }
            retire_wiped(&mut g.pool).unwrap();
            let next = next_generation(&g.pool).unwrap();
            assert_eq!(
                (next.cash, next.shares, next.sol_reserve, next.sol_index),
                (0, 0, 0, [0; 4])
            );
            gens.push(Generation {
                pool: next,
                lenders: [Lender::new(next.generation); 4],
                loans: vec![],
            });
            r.generations += 1;
            if gens.len() > 16 {
                finish(&mut gens[0], &mut r);
                gens.remove(0);
            }
        }
        invariants(&gens, &r);
    }
    // Fully repay the remaining live loans, then withdraw all generations: every
    // actual paid lamport and remaining raw token must be accounted for exactly.
    for g in &mut gens {
        while let Some(q) = g.loans.pop() {
            repay(&mut g.pool, q).unwrap();
        }
        finish(g, &mut r);
    }
    invariants(&gens, &r);
    assert_eq!(r.claimed_sol, r.paid_sol);
    r
}
#[test]
fn model_sequences_cover_randomized_loans_defaults_shares_and_generations() {
    for seed in 1..=64 {
        let r = run_model(seed, 1000);
        assert!(r.generations > 10);
        assert!(r.successes > 100);
        assert!(r.rejected > 100);
    }
}

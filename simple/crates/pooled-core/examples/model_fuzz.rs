//! Reproducible bounded host model runner; never implies on-chain qualification.
#[path = "../tests/model_sequences.rs"]
mod model;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = args
        .get(1)
        .map_or(Ok(103410000), |s| s.parse())
        .expect("seed u64");
    let steps = args
        .get(2)
        .map_or(Ok(20000), |s| s.parse())
        .expect("steps u64");
    let r = model::run_model(seed, steps);
    println!(
        "{{\"scope\":\"pure-core-model-only\",\"seed\":{},\"steps\":{},\"successes\":{},\"rejected\":{},\"generations\":{},\"deposits\":{},\"donations\":{},\"withdrawn_tokens\":{},\"defaulted_tokens\":{},\"paid_sol\":{},\"claimed_sol\":{}}}",
        r.seed,
        r.steps,
        r.successes,
        r.rejected,
        r.generations,
        r.deposits,
        r.donations,
        r.withdrawn_tokens,
        r.defaulted_tokens,
        r.paid_sol,
        r.claimed_sol
    );
}

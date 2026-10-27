#[allow(dead_code)]
#[path = "sources/tests/fuzz/src/rng.rs"]
mod rng;
fn main() {
    let names = ["mock", "mixed", "curve", "pumpswap", "amm-v4", "cpmm"];
    println!("worker,ordinal,seed,splitmix64,index,profile");
    for job in 0u64..2 {
        for k in 0u64..40 {
            let seed = 103410000u64.wrapping_add(job).wrapping_add(k.wrapping_mul(2));
            let hash = rng::splitmix64(seed);
            let index = (hash % names.len() as u64) as usize;
            println!("{},{},{},{},{},{}",job,k+1,seed,hash,index,names[index]);
        }
    }
}

//! Tiny compile fixture for shell terminal benchmarks.
fn main() {
    let n: u64 = (0u64..10_000).map(|i| i.wrapping_mul(i)).sum();
    // Prevent total dead-code elimination while keeping the crate tiny.
    if std::env::var_os("RMUX_BENCH_PRINT").is_some() {
        println!("{n}");
    }
}

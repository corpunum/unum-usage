//! Micro-bench: cargo run --release --example bench
use std::time::Instant;
fn main() {
    let dir = std::env::temp_dir().join(format!("unum-usage-bench-{}", std::process::id()));
    std::env::set_var("UNUM_USAGE_DIR", &dir);
    let n = 200_000u64;
    let t = Instant::now();
    for i in 0..n {
        usage::count("bench", "fn", "hot_path", i % 7 != 0, 3);
    }
    let c = t.elapsed();
    let t = Instant::now();
    for _ in 0..n {
        let _s = usage::span("bench", "route", "GET /api/items");
    }
    let s = t.elapsed();
    usage::flush();
    println!("count(): {:.0} ns/call", c.as_nanos() as f64 / n as f64);
    println!("span():  {:.0} ns/call", s.as_nanos() as f64 / n as f64);
    let t = Instant::now();
    for _ in 0..n {
        std::hint::black_box(Instant::now());
    }
    println!(
        "(Instant::now baseline: {:.0} ns)",
        t.elapsed().as_nanos() as f64 / n as f64
    );
    let _ = std::fs::remove_dir_all(&dir);
}

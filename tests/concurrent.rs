#[test]
fn concurrent_threads_no_loss() {
    let d = std::env::temp_dir().join(format!("uu-conc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::env::set_var("UNUM_USAGE_DIR", &d);
    let hs: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| (0..500).for_each(|_| usage::count("conc", "fn", "t", true, 1)))
        })
        .collect();
    hs.into_iter().for_each(|h| h.join().unwrap());
    usage::flush();
    let n = std::fs::read_to_string(d.join("conc.jsonl"))
        .unwrap()
        .lines()
        .count();
    assert_eq!(n, 4000);
    let _ = std::fs::remove_dir_all(&d);
}

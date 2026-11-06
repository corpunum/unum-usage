use std::fs;
#[test]
fn rolls_over_at_limit() {
    let d = std::env::temp_dir().join(format!("uu-roll-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    std::env::set_var("UNUM_USAGE_DIR", &d);
    std::env::set_var("UNUM_USAGE_MAX_BYTES", "2000");
    for i in 0..100 {
        usage::count("roll", "fn", &format!("f{i}"), true, 1);
        usage::flush();
    }
    assert!(d.join("roll.jsonl.1").exists());
    assert!(fs::metadata(d.join("roll.jsonl")).unwrap().len() < 4000);
    let _ = fs::remove_dir_all(&d);
}

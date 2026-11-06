#[test]
fn off_writes_nothing() {
    let d = std::env::temp_dir().join(format!("uu-off-{}", std::process::id()));
    std::env::set_var("UNUM_USAGE", "off");
    std::env::set_var("UNUM_USAGE_DIR", &d);
    usage::count("off", "cmd", "x", true, 1);
    usage::span("off", "cmd", "y").finish();
    usage::flush();
    assert!(!d.exists());
}

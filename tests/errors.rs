#[test]
fn unwritable_dir_is_swallowed() {
    // dir path is under a regular file: create_dir_all fails
    let f = std::env::temp_dir().join(format!("uu-err-{}", std::process::id()));
    std::fs::write(&f, b"x").unwrap();
    std::env::set_var("UNUM_USAGE_DIR", f.join("sub"));
    for i in 0..50_000 {
        usage::count("err", "fn", "x", i % 2 == 0, 1);
    }
    usage::flush();
    usage::cmd_start("err", "c");
    usage::cmd_end(false);
    let _ = std::fs::remove_file(&f);
}

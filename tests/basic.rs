use std::fs;
fn dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("uu-basic-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}
#[test]
fn writes_exact_format_and_span() {
    let d = dir();
    std::env::set_var("UNUM_USAGE_DIR", &d);
    usage::count("My Engine!", "cmd", "status", true, 5);
    {
        let mut t = usage::span("eng", "route", "GET /a/{id}");
        t.fail();
    }
    {
        let _t = usage::span("eng", "weird-kind", "x\ny\"z");
    }
    usage::flush();
    let a = fs::read_to_string(d.join("my_engine_.jsonl")).unwrap();
    let v: serde_json::Value = serde_json::from_str(a.trim()).unwrap();
    assert_eq!(v["engine"], "my_engine_");
    assert_eq!(v["kind"], "cmd");
    assert_eq!(v["name"], "status");
    assert_eq!(v["ok"], true);
    assert_eq!(v["ms"], 5);
    let ts = v["ts"].as_str().unwrap();
    assert!(
        ts.len() == 24 && ts.ends_with('Z') && ts.as_bytes()[10] == b'T',
        "{ts}"
    );
    let b = fs::read_to_string(d.join("eng.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = b
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["ok"], false);
    assert_eq!(rows[0]["kind"], "route");
    assert_eq!(rows[1]["kind"], "fn");
    assert_eq!(rows[1]["name"], "xy\"z");
    let _ = fs::remove_dir_all(&d);
}

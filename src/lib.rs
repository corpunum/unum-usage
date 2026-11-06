//! unum-usage: tiny, local-only usage counters.
//!
//! ```no_run
//! usage::count("myengine", "cmd", "status", true, 12);
//! let t = usage::span("myengine", "route", "GET /api/items");
//! // ... work ...; on failure: t.fail();
//! drop(t); // records ms and ok
//! ```
//!
//! Appends one JSON line per event to `~/.local/share/unum-usage/<engine>.jsonl`:
//! `{"ts","engine","kind","name","ok","ms"}`. Counts only: never args, content or
//! personal data. No network. All I/O failures are swallowed; nothing here panics
//! or blocks the caller on I/O (lines are buffered in memory and flushed in
//! batches, using a non-blocking file lock; if the lock is busy the batch is kept
//! for the next attempt, bounded in size).
//!
//! Env: `UNUM_USAGE=off` disables everything. `UNUM_USAGE_DIR` overrides the
//! directory. `UNUM_USAGE_MAX_BYTES` overrides the 20 MB rollover (tests).
//! Short-lived processes should call [`flush`] (or use [`cmd_exit`]) before exit.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Rollover size: when a file reaches this, it is renamed to `<engine>.jsonl.1`.
pub const MAX_BYTES: u64 = 20 * 1024 * 1024;
const FLUSH_BYTES: usize = 8 * 1024;
const FLUSH_EVERY: Duration = Duration::from_millis(1000);
/// Upper bound of unflushed bytes held per engine when the file is unwritable.
const BUF_CAP: usize = 256 * 1024;
const RETRY_AFTER: Duration = Duration::from_secs(5);
const NAME_MAX: usize = 96;

struct State {
    bufs: HashMap<String, String>,
    last_flush: Instant,
    total: usize,
    retry_at: Option<Instant>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(State {
            bufs: HashMap::new(),
            last_flush: Instant::now(),
            total: 0,
            retry_at: None,
        })
    })
}

/// True unless `UNUM_USAGE=off` (also `0`, `false`, `no`, `disabled`).
pub fn enabled() -> bool {
    static E: OnceLock<bool> = OnceLock::new();
    *E.get_or_init(|| {
        !matches!(
            std::env::var("UNUM_USAGE")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "off" | "0" | "false" | "no" | "disabled"
        )
    })
}

fn dir() -> Option<std::path::PathBuf> {
    if let Ok(d) = std::env::var("UNUM_USAGE_DIR") {
        if !d.is_empty() {
            return Some(d.into());
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(".local/share/unum-usage"))
}

fn max_bytes() -> u64 {
    std::env::var("UNUM_USAGE_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(MAX_BYTES)
}

fn clean_engine(e: &str) -> String {
    let s: String = e
        .chars()
        .take(48)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "unknown".into()
    } else {
        s
    }
}

fn clean_kind(k: &str) -> &'static str {
    match k {
        "cmd" => "cmd",
        "route" => "route",
        "mcp" => "mcp",
        _ => "fn",
    }
}

/// RFC 3339 UTC with milliseconds, e.g. `2026-11-06T12:34:56.789Z`.
fn ts_now() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let ms = d.subsec_millis();
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn line(engine: &str, kind: &str, name: &str, ok: bool, ms: u64) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX)
        .collect();
    let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    format!(
        "{{\"ts\":\"{}\",\"engine\":{},\"kind\":\"{}\",\"name\":{},\"ok\":{},\"ms\":{}}}\n",
        ts_now(),
        q(engine),
        kind,
        q(&name),
        ok,
        ms
    )
}

/// Record one event. Cheap (in-memory append); never panics, never does blocking I/O
/// on the hot path except an occasional batched flush.
pub fn count(engine: &str, kind: &str, name: &str, ok: bool, ms: u64) {
    if !enabled() {
        return;
    }
    let engine = clean_engine(engine);
    let l = line(&engine, clean_kind(kind), name, ok, ms);
    let due = {
        let Ok(mut st) = state().lock() else { return };
        let buf = st.bufs.entry(engine).or_default();
        if buf.len() < BUF_CAP {
            buf.push_str(&l);
            st.total += l.len();
        }
        let backoff = st.retry_at.is_some_and(|t| Instant::now() < t);
        !backoff && (st.total >= FLUSH_BYTES || st.last_flush.elapsed() >= FLUSH_EVERY)
    };
    ensure_flusher();
    if due {
        flush();
    }
}

fn ensure_flusher() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    // Idle servers: push leftovers out within ~1s. Detached; never blocks exit.
    let _ = std::thread::Builder::new()
        .name("unum-usage-flush".into())
        .spawn(|| loop {
            std::thread::sleep(FLUSH_EVERY);
            flush();
        });
}

/// Write buffered lines now. Safe to call anywhere, any number of times.
pub fn flush() {
    let taken: Vec<(String, String)> = {
        let Ok(mut st) = state().lock() else { return };
        st.last_flush = Instant::now();
        let v: Vec<_> = st.bufs.drain().filter(|(_, b)| !b.is_empty()).collect();
        st.total = 0;
        v
    };
    for (engine, data) in taken {
        if let Err(data) = write_batch(&engine, data) {
            // lock busy / I/O error: keep (bounded) for the next attempt
            if let Ok(mut st) = state().lock() {
                // back off so a broken dir/lock costs nothing on the hot path
                st.retry_at = Some(Instant::now() + RETRY_AFTER);
                let b = st.bufs.entry(engine).or_default();
                if b.len() + data.len() <= BUF_CAP {
                    b.insert_str(0, &data);
                    st.total += data.len();
                }
            }
        }
    }
}

fn write_batch(engine: &str, data: String) -> Result<(), String> {
    let Some(d) = dir() else { return Err(data) };
    if fs::create_dir_all(&d).is_err() {
        return Err(data);
    }
    let Ok(lock) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(d.join(format!("{engine}.lock")))
    else {
        return Err(data);
    };
    let mut locked = false;
    for _ in 0..5 {
        if lock.try_lock().is_ok() {
            locked = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if !locked {
        return Err(data);
    }
    let path = d.join(format!("{engine}.jsonl"));
    if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) >= max_bytes() {
        let _ = fs::rename(&path, d.join(format!("{engine}.jsonl.1")));
    }
    let r = (|| -> std::io::Result<()> {
        let mut f: File = OpenOptions::new().create(true).append(true).open(&path)?;
        f.write_all(data.as_bytes())
    })();
    let _ = lock.unlock();
    r.map_err(|_| data)
}

/// Timer guard: records ms and ok on drop. Mark failure with [`Span::fail`].
#[must_use = "bind the span to a variable (`let _t = ...`) so it lives to the end of the scope"]
pub struct Span<'a> {
    engine: &'a str,
    kind: &'a str,
    name: &'a str,
    start: Instant,
    ok: bool,
    done: bool,
}

/// Start timing a `kind` event named `name`. Counts as ok unless `.fail()` is called.
pub fn span<'a>(engine: &'a str, kind: &'a str, name: &'a str) -> Span<'a> {
    Span {
        engine,
        kind,
        name,
        start: Instant::now(),
        ok: true,
        done: false,
    }
}

impl Span<'_> {
    /// Mark this event as failed.
    pub fn fail(&mut self) {
        self.ok = false;
    }
    /// Set outcome explicitly.
    pub fn set_ok(&mut self, ok: bool) {
        self.ok = ok;
    }
    /// Record now (and flush) instead of on drop; useful right before `process::exit`.
    pub fn finish(mut self) {
        self.record();
        flush();
    }
    fn record(&mut self) {
        if !self.done {
            self.done = true;
            count(
                self.engine,
                self.kind,
                self.name,
                self.ok,
                self.start.elapsed().as_millis() as u64,
            );
        }
    }
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        self.record();
    }
}

static CMD: Mutex<Option<(String, String, Instant)>> = Mutex::new(None);

/// For CLIs: remember the running subcommand so [`cmd_exit`]/[`cmd_end`] can record it.
pub fn cmd_start(engine: &str, name: &str) {
    if let Ok(mut c) = CMD.lock() {
        *c = Some((engine.to_string(), name.to_string(), Instant::now()));
    }
}

/// Record the subcommand started by [`cmd_start`] (kind `cmd`) and flush.
pub fn cmd_end(ok: bool) {
    let c = CMD.lock().ok().and_then(|mut c| c.take());
    if let Some((e, n, t)) = c {
        count(&e, "cmd", &n, ok, t.elapsed().as_millis() as u64);
    }
    flush();
}

/// Record the subcommand (ok = code 0), flush, then `std::process::exit(code)`.
pub fn cmd_exit(code: i32) -> ! {
    cmd_end(code == 0);
    std::process::exit(code)
}

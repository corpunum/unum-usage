# unum-usage

Tiny, local-only usage counters for the Unum engines. Std plus `serde_json`; no async runtime, no network.

```rust
usage::count("myengine", "cmd", "status", true, 12);   // engine, kind, name, ok, ms
let mut t = usage::span("myengine", "route", "GET /api/items");
if bad { t.fail(); }                                  // ok=false; ms and ok recorded on drop
```

Kinds: `cmd`, `route`, `mcp`, `fn` (anything else is stored as `fn`).
For CLIs: `usage::cmd_start(engine, name)` at dispatch and `usage::cmd_exit(code)` / `usage::cmd_end(ok)` at the end (records and flushes). Long-lived servers need nothing extra; short-lived processes should call `usage::flush()` before exit.

## Output

Appends JSON lines to `~/.local/share/unum-usage/<engine>.jsonl`:

```json
{"ts":"2026-11-06T12:34:56.789Z","engine":"unumship","kind":"cmd","name":"ship","ok":true,"ms":412}
```

`ts` is RFC 3339 UTC with milliseconds. Counts only: the caller passes a fixed name (a subcommand, a route pattern such as `GET /api/items/{id}`, a tool name); never arguments, content or personal data. Names are truncated to 96 characters.

- Buffered in memory, flushed in batches (8 KB, 1 s, or `flush()`), under a non-blocking file lock (`<engine>.lock`), O_APPEND writes.
- Rolls over at 20 MB to `<engine>.jsonl.1` (one generation kept).
- Never panics; all I/O errors are swallowed; the in-memory buffer is bounded (256 KB per engine).
- Env: `UNUM_USAGE=off` disables; `UNUM_USAGE_DIR` overrides the directory.

Needs Rust 1.89+ (std file locking). Micro-bench: `cargo run --release --example bench`.

License: Apache-2.0.

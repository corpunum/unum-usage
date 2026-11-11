# Handover (living document)

Update this file with every change that alters state, open work or how to ship. Newest state on top.

## State (2026-11-11)

- Version **0.1.0**, one commit, CI green (Linux: build, tests). Library name `usage`, 469 lines,
  std + `serde_json` only.
- Consumers pin a git revision (`unum-usage = { git = "...", rev = "8e521e1..." }`): unumsearch,
  unumbeat, unumship, UnumHub. Changing this crate does nothing for them until they move the pin.
- Not on crates.io. This **blocks `cargo publish` of every consumer**: crates.io refuses a
  dependency that has only a `git` source (`cargo publish --dry-run` in unumsearch fails with
  "dependency `unum-usage` does not specify a version").

## How to ship

1. Detached worktree of `origin/main`, change, gates: `cargo fmt --check && cargo clippy
   --all-targets -- -D warnings && cargo test`.
2. One push to `main`, CI green.
3. Tell the consumers: bump their pinned `rev` (and `Cargo.lock`) in their next bundle.

## Open work

- **Publish 0.1.0 to crates.io** (owner step: `cargo login`, then `cargo publish`; check the name
  is free first). Consumers then declare `version = "0.1"` next to the git pin, or drop the pin.
- Per-minute aggregation for routes above about 1,000 calls a day (master plan RS-08): the hub
  wrote 58,924 rows in four days, almost all of them two polling routes.
- A row schema file (JSON Schema) so the OpenUnum collector and this crate cannot drift (AR-04).
- Planned home: a shared `unum-kit` workspace (AR-05); keep the crate name and the `usage::` API
  when it moves.

## Traps

- The crate must never panic or block its host: all I/O errors are swallowed and the buffer is
  bounded. Keep it that way; tests cover an unwritable directory, concurrent writers and rollover.
- Names are recorded as given. Callers pass route patterns (`GET /api/items/{id}`), never the
  concrete path, arguments or content.

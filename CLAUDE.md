# False Colours

Single-player Age-of-Sail sandbox game. Rust + Bevy 0.20, 3D with a top-down camera, macOS first (Windows and Linux builds too).

**Read before any change:**
- `docs/CODING_STANDARDS.md`: mandatory rules (one hot path, no duplicated logic, KISS, testing).
- `docs/DESIGN.md`: game vision, factions, systems, roadmap.

Also: `docs/ASSET_SOURCES.md`, where to find free models, sounds, music and ship plans, and their licences; `docs/MODELS.md`, the spec and workflow for 3D models (run it with `/ship-model <class>`).

## Commands

```sh
cargo run --features dev                       # run with fast builds and the full chronicle
cargo test --workspace --features chronicle    # all tests, the tools' too
cargo clippy --workspace --all-targets -- -D warnings   # lint (also with --features chronicle)
cargo fmt --all --check                        # format check
cargo soak --days 100                          # the world running on its own, headless, logged
cargo logbook                                  # the logs as web pages, at http://127.0.0.1:7716
cargo logbook --summary                        # the newest run's summary, as text
```

The repo is a Cargo workspace: the game at the root, `crates/chronicle` (the
log's record types, shared), and `tools/report` (the `cargo logbook` server).
Plain `cargo run`/`test`/`build` mean the game alone.

Playtest logs: with the `chronicle` feature (on with `dev`) every run writes
a log to the logs folder beside the quicksave, and F8 takes a note. Every
build writes crash reports to the crashes folder. After a change, run
`cargo soak` and compare it with the run before in `cargo logbook`.

CI: `.github/workflows/ci.yml` runs the format check, clippy, and the tests on
every push and pull request.

Releases: push a `v*` tag and `.github/workflows/release.yml` builds macOS
(universal `.app`), Windows and Linux downloads and publishes them as a GitHub
release. Running it by hand from the Actions tab builds without releasing.

## Key rules (summary; the standards doc is authoritative)

- Search for existing code before writing any helper. Never duplicate logic across files.
- Simulation runs in `FixedUpdate`; input/rendering in `Update`. Pause = pause `Time<Virtual>`.
- Pure logic as plain functions with unit tests; systems are thin adapters.
- Coordinates: Y-up, ocean on XZ, North = −Z, East = +X, headings in radians clockwise from north. Conversions live only in `src/math.rs`.

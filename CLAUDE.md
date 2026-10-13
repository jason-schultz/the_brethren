# The Brethren

Single-player Age-of-Sail sandbox game. Rust + Bevy 0.20, 3D with a top-down camera, macOS.

**Read before any change:**
- `docs/CODING_STANDARDS.md`: mandatory rules (one hot path, no duplicated logic, KISS, testing).
- `docs/DESIGN.md`: game vision, factions, systems, roadmap.

## Commands

```sh
cargo run --features dev                       # run with fast incremental builds
cargo test                                     # all tests
cargo clippy --all-targets -- -D warnings      # lint
cargo fmt --check                              # format check
```

## Key rules (summary; the standards doc is authoritative)

- Search for existing code before writing any helper. Never duplicate logic across files.
- Simulation runs in `FixedUpdate`; input/rendering in `Update`. Pause = pause `Time<Virtual>`.
- Pure logic as plain functions with unit tests; systems are thin adapters.
- Coordinates: Y-up, ocean on XZ, North = −Z, East = +X, headings in radians clockwise from north. Conversions live only in `src/math.rs`.

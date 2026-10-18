# Coding Standards

These rules are **not optional**. They apply to every change, whether a
human or an AI agent wrote it. Code review rejects changes that violate
them.

---

## 1. One Hot Path

Every behavior has **one** canonical code path. There is one way to compute
a ship's speed, one way to convert between headings and vectors, and one way
to issue a movement order.

- **Deviating from the hot path requires a documented reason.** Put a
  comment at the deviation explaining *why* the canonical path can't be used.
  "It was easier" is not a reason. Valid reasons look like a measured
  performance problem or a genuinely different requirement.
- **If the canonical path doesn't fit, change the canonical path.** Don't
  fork it.

## 2. No Duplicated Logic (DRY)

AI agents are prone to re-writing private helpers that already exist
elsewhere. Don't.

- **Search before you write.** Before adding any function, `grep` the
  codebase for existing code that does the same thing, including under
  different names.
- **No near-duplicates.** Two functions that differ only by a constant, a
  type, or a sign get merged into one function with a parameter.
- **Shared code has a home.** If two modules need the same helper, it moves
  to the module that owns the concept (for example, angle math lives in
  `math`), not into a `utils` junk drawer and not copied into both.
- **Constants are defined once.** Tunables live with the concept they tune,
  as named `const`s or in a config resource. They are never repeated as
  magic numbers.

## 3. Keep It Simple (KISS)

- Write the simplest code that correctly does the job. Don't build
  speculative abstractions, traits with a single implementation, generic
  parameters "for later", or builder patterns for three fields.
- Prefer plain functions over methods over traits over macros.
- No spaghetti. Data flows in one direction: **input → orders → simulation
  → presentation**. Presentation never mutates simulation state, and the
  simulation never reads raw input.
- If a function needs a paragraph to explain what it does, split it.

## 4. Project Structure

```
src/
  main.rs          # App construction only: adds plugins, nothing else
  lib.rs           # Declares modules; SimulationPlugin and GamePlugin
  math.rs          # Angle/heading conventions, unit conversions, plane mapping
  geometry.rs      # 2D polygon algorithms on the ocean plane
  schedule.rs      # Shared SystemSets and their ordering
  ship_class.rs    # Ship classes: the one way to build a ship
  <domain>/        # Simulation domains: sailing, navigation, map, economy, ...
    mod.rs         #   The domain's Plugin, components, messages, systems
    *.rs           #   Pure logic (physics, course, route) with unit tests
  <domain>.rs      # Small domains in one file: clock, goods, cargo, crew, port
  visuals/         # 3D presentation: reads the simulation, never writes it
  ui/              # Screen UI: changes the simulation only by sending messages
assets/maps/       # Map data (RON)
assets/data/       # Other game data (RON): goods
```

- **The simulation can run headless.** `SimulationPlugin` must work with
  `MinimalPlugins` and nothing else: no rendering, windows, or input.
  Presentation lives in `visuals/` and `ui/`, and input in `player.rs`,
  `camera.rs`, and `time_control.rs`.
- **One plugin per domain.** A domain owns its components, resources,
  messages, and systems. Other domains depend on its public types, never on
  its internals. Top-level domain plugins are `struct`s. Private
  sub-modules inside `visuals/` and `ui/` use `pub(super) fn plugin(app: &mut App)`.
- **Pure logic is separate from systems.** Math and rules, such as the
  points-of-sail speed curve or route planning, are plain functions or types
  with no Bevy `Query` or `Res` parameters. Systems are thin adapters that
  fetch data, call the pure function, and write the result back. This keeps
  logic trivially testable.
- **Rules are checked in one place.** When the UI must know whether an
  order will succeed (to show or hide a button), it calls the same function
  the simulation uses to validate that order, such as `market::quote` or
  `jobs::check_acceptance`. Never re-implement a rule for display.
- **Quantities that accumulate use `f64`.** Stock, cargo, and the clock
  change by tiny amounts each fixed step, and `f32` silently drops them.
  Positions and physics stay `f32`.
- **Visibility is minimal.** Items are private unless another module needs
  them. Use `pub(crate)` over `pub` unless the item is part of the
  library's API.

## 5. Rust

- `cargo fmt` clean. `cargo clippy --all-targets -- -D warnings` clean.
- No `unsafe` (enforced by `forbid` in `Cargo.toml`).
- No `unwrap()`/`expect()` in game code except where an invariant makes
  failure impossible, and then use `expect("why this can't fail")`. In
  systems, prefer `let Ok(x) = query.single() else { return };` or Bevy's
  fallible system params.
- Use strong types for units where it prevents real bugs. Document units in
  names or doc comments, for example `speed_mps` or `heading` (radians).
- Prefer iterators and pattern matching. Don't `clone()` to satisfy the
  borrow checker without understanding why.
- Use doc comments (`///`) on every public item. Inline comments explain
  *why*, not *what*.

## 6. Bevy

- **Simulation in `FixedUpdate`**, presentation and input in `Update`.
  Pausing works by pausing `Time<Virtual>`, never by sprinkling
  `if paused { return }` checks through systems.
- **Explicit ordering** with `SystemSet`s, not implicit or accidental
  ordering. Shared sets are declared in one place (`schedule.rs`).
- **Messages (events) for cross-domain communication.** Orders, collisions,
  and damage are messages. Domains don't reach into each other's components
  to trigger behavior.
- **Small, focused components.** Prefer composition over large "god"
  components. Use required components to guarantee bundles are complete.
- **Use `Res`/`ResMut` deliberately.** Take `ResMut` only when the system
  mutates the resource, because it serializes scheduling.
- **No per-frame allocation in hot systems** without a reason.
- **Assets** are loaded through `AssetServer` or created once at startup.
  Never create meshes or materials per frame.

## 7. Game Development

- **Coordinates:** Bevy's right-handed, Y-up. The ocean is the XZ plane at
  `y = 0`. **North is −Z, East is +X.** Headings are radians, measured
  clockwise from north (compass convention). Conversion lives *only* in
  `math`.
- **Units:** meters, seconds, radians, meters/second internally. Convert to
  knots or degrees only for display.
- **Frame-rate independence:** all simulation uses the fixed timestep's
  `delta_secs()`. Never use per-frame constants.
- **Step-size independence:** time compression stretches the fixed step up
  to `time_scale::MAX_STEP` (2 s). Simulation code must behave the same at
  1/64 s and at 2 s, and long-voyage tests run at `MAX_STEP`. Anything that
  fires on a threshold uses `alert::fell_below` on the crossing, so it fires
  once whatever the step.
- **Data-driven content:** ship classes, goods, ports, and factions will be
  defined as data (RON assets), not hard-coded match arms, once they exist.
- **Determinism where cheap:** simulation should not depend on frame rate
  or system iteration order. Use seeded RNG when randomness arrives.

## 8. Testing

Everything is tested, and tests must be **valid and meaningful**:

- **Test behavior, not implementation.** Assert on outcomes a player or
  designer would recognize ("a ship can't make headway directly into the
  wind"), not on private intermediate values.
- **Pure logic gets unit tests** in a `#[cfg(test)] mod tests` beside the
  code. Cover edge cases: angle wraparound, zero wind, zero speed, and
  boundary values.
- **Systems get integration tests** using a headless `App` with
  `MinimalPlugins` plus the domain plugin under test. Advance time, then
  assert on the world.
- **No tautological tests.** A test that re-implements the function under
  test to compute its expected value is invalid. Use known values, physical
  invariants, or properties (symmetry, monotonicity).
- **No sleeping, no real time.** Drive time manually.
- **Bugs get a regression test first,** written to fail before the fix.
- `cargo test` must pass before any change is considered done.

## 9. Definition of Done

A change is done when:

1. It follows the hot path, or documents why it doesn't.
2. You searched for duplicates and found none.
3. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` all pass.
4. New behavior has tests that would fail if the behavior broke.
5. `docs/DESIGN.md` reflects any design change.

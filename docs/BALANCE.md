# Balance: knobs and targets

How False Colours is tuned, and how we tell whether it's tuned well.

## Two kinds of number

- **Knobs** (`assets/data/tuning.ron`) change how the game plays: how far
  a lookout sees, how fast a holed hull fills, when a captain strikes.
  The game builds them in, so a change takes effect on the next
  `cargo soak` or `cargo run`.
- **Targets** (`assets/data/balance.ron`) change nothing in the game.
  They say what a good soak looks like, as a range for one measure:
  "1 to 8 ships founder per 100 days". Each soak is measured against
  them, and its summary lists every target it missed.

Every knob and target carries hints: what it does, how raising or
lowering it changes play, and the history behind it. Where the game
departs from history for the sake of play, the hint says so and why.

## Tuning from the logbook

`cargo logbook`, then the **Tuning** page (<http://127.0.0.1:7716/tuning>):

- Each target, with its range, the newest soak's measure, and whether it's
  on target. Leave a bound empty for no least or most.
- Each knob, with its value, the range it makes sense in, and the
  historical value where it differs.
- Saving writes the RON file; a value outside its range is refused.

Then run `cargo soak --days 100` and look at the run's summary (or the
Tuning page again) to see what moved.

## The loop

1. Decide what should feel different, in words: "battered ships should
   limp home, not sink on the spot."
2. Find the target that measures it, or add one (below).
3. Turn the knobs its hints name, a little at a time.
4. Soak, and check the targets. Compare the run with the one before.

Change one knob at a time where you can: two moving at once make it hard
to tell which did what. Soaks vary by seed; a target missed by a hair in
one soak may be met in the next.

## The measures

| Target id | What it measures |
|---|---|
| `prizes_per_100_days` | Fights that ended with a ship taken, per 100 days |
| `broke_off_share` | The share of fights broken off |
| `gone_down_per_100_days` | Ships sunk, foundered, or wrecked, per 100 days |
| `foundered_per_100_days` | Ships the water gained on, per 100 days |
| `most_fights_one_pair` | The most fights between the same two ships: a loop detector |
| `ports_changed_per_100_days` | Ports going over to another nation, per 100 days |

The starting ranges are proposals, for you to set.

## Adding a knob or a target

- **A knob:** add an entry to `tuning.ron`, a field to `Tuning` in
  `src/tuning.rs` read from it, and use the field where the constant was.
  The game's tests fail if a knob it needs is missing or out of range.
- **A target:** add an entry to `balance.ron` and its measure to
  `measure` in `tools/report/src/balance.rs`, from what the report
  folds out of the log. A test fails if a target has no measure.

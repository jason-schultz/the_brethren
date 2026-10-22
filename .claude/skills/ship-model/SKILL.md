---
name: ship-model
description: Build or rework a False Colours ship model in Blender, step by step with the user's approval at each step. Use when asked to model, remodel, or fix a ship class (sloop, schooner, brig, fluyt, frigate, longboat, brigantine), e.g. "/ship-model sloop".
---

# Ship model

Build the ship class named in the arguments (e.g. `sloop`), following
`docs/MODELS.md`. Read it first: it holds the spec (scale, axes, parts,
material slots, export) and the reasons. This skill is the procedure.

## Before starting

- Read `docs/MODELS.md` and the class's entry in `assets/data/ships.ron`
  (length, rig).
- Check the Blender MCP connection works (a scene summary is enough). If
  it doesn't, stop and tell the user how to connect it; don't write
  scripts blind.
- If `tools/blender/ships.py` exists, build with it: change the generator
  or the class's data, never hand-model a one-off. If it doesn't exist
  yet, this run writes it, general enough for every class.

## Steps

Do one step at a time. End each with what it produced (a table or
screenshots) and wait for the user's yes before the next. Never run
ahead.

1. **References.** Look in `tools/blender/references/<class>/` for plans
   the user saved. If none, search the sources in
   `docs/ASSET_SOURCES.md` ("Ship plans and references"), download
   candidates into the scratchpad, look at them, and propose the best
   (side profile, half-breadth plan, body plan, a picture of the rig).
   Save the chosen ones to `tools/blender/references/<class>/` with a
   `SOURCES.md` noting each one's URL and licence.
2. **Numbers.** Read the plan and write the class's entry in
   `tools/blender/ships.json`: length (must equal `ships.ron`), beam,
   depth, draught, sheer, the table of offsets (10–20 stations), masts
   (position, height, rake, yards), bowsprit, gun ports. Show it as a
   table, with the plan's scale and how it was read. No mesh yet.
3. **Hull.** Generate it. Screenshots: side, top, and bow, with the plan
   as a reference image behind where it helps.
4. **Deck and masts.** Decks, masts, yards, booms, bowsprit. Screenshots.
5. **Sails and flag.** Named per the spec. Screenshots, set and furled.
6. **Materials and UVs.** The spec's slots in flat colours; unwrap every
   part. Screenshot, and the hull's UV layout.
7. **Export and check.** Export `assets/models/ships/<class>.glb`. Check
   against the spec: scale, bow toward −Z, origin, part and slot names,
   triangle count. Screenshot from the game camera's height and angle.

## Rules

- Screenshots cost compute: take the ones each step names, not more.
- If the user says something looks wrong, fix it in the generator or
  the data, so every class gets the fix.
- If a step keeps failing or wasting compute, say so and propose a
  change to this skill and `docs/MODELS.md`.
- Don't touch the game's code (`src/visuals/ships.rs`) in this skill:
  loading the models is separate work.
- Never commit; the user does.

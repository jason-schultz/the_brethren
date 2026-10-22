# Models

How the game's 3D models are made, what they must look like on the way
in, and the workflow for building them in Blender. Where to find plans,
textures, and other assets: `ASSET_SOURCES.md`.

## The plan

Ships first, one model per class in `assets/data/ships.ron`, built by one
generator script from data. Towns later, likely from a CC0 pack.

| Class | Rig | Length | Masts |
|---|---|---|---|
| Longboat | Lugger | 8 m | 1, lug sail |
| Schooner | Schooner | 18 m | 2, fore-and-aft |
| Sloop | Sloop | 20 m | 1, fore-and-aft |
| Brigantine | Brigantine | 25 m | 2, square fore, fore-and-aft main |
| Brig | Brig | 28 m | 2, square |
| Fluyt | Ship | 32 m | 3, square |
| Frigate | Ship | 36 m | 3, square |

Lengths come from `ships.ron`, the one source of truth; the generator's
data must match them.

The order: the sloop first, checked in the game's own camera, then the
rest from the same script. Wiring the models into `src/visuals/ships.rs`
in place of the placeholders is separate work, done once they're
approved.

## One generator, many ships

Ships aren't modelled by hand. One Blender Python script builds any ship
from a set of numbers, so a fix to the bow fixes every ship and remaking
all of them is cheap.

- `tools/blender/ships.py`: the generator. Run inside Blender (through
  the MCP connection or by hand); builds a class, or all of them, and
  exports each.
- `tools/blender/ships.json`: one entry per class: length, beam, depth
  of hold, draught, sheer, the table of offsets, masts, gun ports.
- `tools/blender/references/`: the plans and pictures each class was
  built from, with a note of where each came from.

### The table of offsets

The hull comes from a table of offsets, the way shipwrights drew them:
the hull's half-breadth at each station along the keel, at each
waterline height. Read from a plan's body plan and half-breadth plan,
10 to 20 stations, the script smooths between them. The table is written
out and approved before any mesh is made: it's the cheapest place to
catch a misread plan.

## The spec

- **Format:** glTF binary (`.glb`), into `assets/models/ships/`, one file
  a class, named after it: `sloop.glb`.
- **Scale:** 1 unit is 1 meter, at the class's true length. (The
  placeholders scale a sloop-sized model; real models won't need it.)
- **Axes:** in the game, Y is up and the bow points along −Z; the
  waterline sits at Y = 0. Blender is Z-up: model with the bow along
  Blender's +Y and let the glTF exporter's "+Y up" convert it. Check the
  first export in the game.
- **Origin:** amidships, on the waterline, on the centerline.
- **Budget:** about 1,000 to 3,000 triangles a ship. The camera is
  top-down and a ship is a few dozen pixels: silhouette, rig, and colour
  read, planking doesn't.
- **Parts**, as separate named objects, so the game can show sail set or
  furled and colour each part:
  - `hull`, `deck`
  - `mast.fore`, `mast.main`, `mast.mizzen` (whichever she has), with
    their yards and booms
  - `sail.<mast>.<sail>`, e.g. `sail.main.course`, `sail.main.topsail`,
    `sail.fore.jib`
  - `flag`
- **Material slots**, named, the same on every ship:

  | Slot | What | Notes |
  |---|---|---|
  | `wood` | hull planking, bare | |
  | `paint` | wales, rails, gun-port lids, stern | livery: tinted per nation in the game |
  | `deck` | decks | |
  | `spars` | masts, yards, booms | |
  | `canvas` | sails | |
  | `flag` | ensign | white, tinted by the game to the colours she flies |
  | `iron` | guns, fittings | |

  Start with flat colours. Textures go on later, slot by slot.
- **UVs:** every part unwrapped cleanly, no overlaps on the hull, so a
  texture can be dropped on later without remodelling. Export the hull's
  UV layout as a PNG beside the model when texturing starts.
- **Materials must survive export.** Blender's procedural node materials
  (Cycles/Eevee) don't export to glTF and won't show in Bevy. Only
  glTF's PBR material does: base colour (a colour or an image),
  roughness, metalness, normal map. Anything procedural is baked to an
  image first.

## Faction, class, and livery

The hull follows the class, not the faction: pirates and navies both
sail sloops. A faction shows in its livery: the `paint` and `flag`
slots, and later stern decoration and the number of gun ports shown.
Variants of a hull (a raider's taller masts, a fluyt's Dutch stern) are
parameters in `ships.json`, not new models.

## The workflow

Followed every time, through the `/ship-model <class>` skill
(`.claude/skills/ship-model/`). Each step ends with a screenshot or a
table, and goes on only with the user's yes.

1. **References.** Find the class's plan (side profile, half-breadth,
   body plan) and a picture of her rig, from the sources in
   `ASSET_SOURCES.md`; save them in `tools/blender/references/`.
2. **Numbers.** Write her entry in `ships.json`: dimensions, offsets,
   masts, from the plan. Shown as a table for approval.
3. **Hull.** Generate it; screenshots from the side, from above, and
   from the bow, with the plan as a reference image behind.
4. **Deck and masts.** Decks, masts, yards, bowsprit.
5. **Sails and flag.**
6. **Materials and UVs.** The slots, in flat colours; unwrap.
7. **Export and check.** Export the `.glb`, check it against the spec,
   and a screenshot from the game's own camera height and angle.

If a step keeps going wrong or wasting compute, change this workflow
(here and in the skill), not just the one ship.

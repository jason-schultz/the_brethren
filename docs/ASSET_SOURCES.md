# Asset Sources

Where to find free models, sounds, music, and textures for False Colours,
and what to check before using them. Collected 2026-10-22; licences on
these sites change, so read each asset's own licence when you download it.

## Licences, in short

| Licence | What it means for us |
|---|---|
| **CC0** / public domain | Use it any way, no credit needed. The safest. |
| **CC-BY** | Free, but credit the author (keep a credits list, see below). |
| **CC-BY-SA** | Credit, and changes to the asset must be shared under the same licence. Usable, but awkward. |
| **NC** (non-commercial) | Avoid: rules out ever selling the game. |
| **ND** (no derivatives) | Avoid: we can't recolour or adapt it. |
| Royalty-free (paid or bundle) | Read the terms; usually fine for games, not for reselling the asset itself. |

Keep a `CREDITS` list of every asset used: name, author, source URL,
licence. CC-BY requires it, and it saves trouble later for CC0 too.

## 3D models

| Site | What's there | Licence |
|---|---|---|
| [Kenney](https://kenney.nl/assets) | Low-poly kits, including a Pirate Kit (ships, docks, buildings) | CC0 |
| [Quaternius](https://quaternius.com) | Low-poly packs, pirate and ship sets among them | CC0 |
| [Poly Pizza](https://poly.pizza) | Search engine for low-poly models | Mostly CC0, some CC-BY |
| [Sketchfab](https://sketchfab.com) | Huge, filter by "Downloadable" and licence | Mixed |
| [itch.io game assets](https://itch.io/game-assets) | Indie packs, free and paid | Mixed |
| [OpenGameArt](https://opengameart.org) | Models, textures, sounds, music | Mixed |
| [CGTrader](https://www.cgtrader.com) / [TurboSquid](https://www.turbosquid.com) | Professional models, mostly paid | Royalty-free |

Tips:
- Pick one pack or artist for everything you can; models from different
  packs rarely match in style.
- Prefer glTF (`.glb` / `.gltf`): Bevy loads it directly. Other formats
  can be converted in Blender.
- The camera is top-down: silhouettes and colour read, fine detail doesn't.

## Ship plans and references

What the ship models are built from (`MODELS.md`). Plans beat paintings:
they're drawn to scale. For each class, look for a **side profile**
(sheer plan), a **half-breadth plan** (from above), a **body plan**
(cross-sections), and a picture of her **rig**. Save what you pick in
`tools/blender/references/<class>/`, with a note of where it came from.

| Site | What's there | Licence |
|---|---|---|
| [Chapman's *Architectura Navalis Mercatoria* on Wikimedia Commons](https://commons.wikimedia.org/wiki/Category:Architectura_Navalis_Mercatoria) | 1768 plates, drawn to scale, of nearly every type we have: sloops, schooners, brigs, fluyts (*flöjt*), frigates, boats | Public domain |
| [Internet Archive](https://archive.org/search?query=Architectura+Navalis+Mercatoria) | Full scans of Chapman, and of Steel's *Elements and Practice of Rigging and Seamanship* (1794) for rigs | Public domain |
| [DigitaltMuseum](https://digitaltmuseum.se) | The Swedish maritime museums' collections, Chapman's own drawings among them; search "Chapman" | Mostly free, check each |
| [Royal Museums Greenwich collections](https://www.rmg.co.uk/collections) | The National Maritime Museum's ship plans and models; search "ship plan" with a type | Images free for non-commercial use; ask for commercial |
| [Wikimedia Commons: ship plans](https://commons.wikimedia.org/wiki/Category:Ship_plans) | Period draughts of many ships | Mostly public domain |
| [Model Ship World](https://modelshipworld.com) | Modellers' forum: plans, rigging detail, photos of builds | Reference only |

Tips:
- Chapman's plates have a scale bar; keep it in the image, it's how the
  measurements are read.
- Our classes in Chapman's terms: sloop (*slup*), schooner (*skonert*),
  brig and brigantine (*brigg*), fluyt (*flöjt*), frigate (*fregatt*),
  longboat (*barkass*).
- Reference images are only looked at, never shipped, so a
  non-commercial image is fine to measure from.

## Textures and materials

| Site | What's there | Licence |
|---|---|---|
| [Poly Haven](https://polyhaven.com) | Textures, HDRIs, some models | CC0 |
| [ambientCG](https://ambientcg.com) | Materials and textures | CC0 |

## Sound effects

| Site | What's there | Licence |
|---|---|---|
| [Sonniss GDC bundles](https://sonniss.com/gameaudiogdc) | Gigabytes of professional effects each year: cannon, water, wood, crowds | Royalty-free for games |
| [Freesound](https://freesound.org) | Enormous library; filter by licence | Mixed, prefer CC0 |
| [Kenney audio](https://kenney.nl/assets/category:Audio) | UI and impact sounds | CC0 |
| [OpenGameArt](https://opengameart.org) | Effects and loops | Mixed |

What matters most for us: the sea (waves, wind by strength, creaking
timber, rigging), harbour ambience (gulls, bells, crowds), cannon and
musket, and a few UI sounds.

## Music

| Site | What's there | Licence |
|---|---|---|
| [Incompetech](https://incompetech.com/music/royalty-free/) | Kevin MacLeod's library, some period-ish pieces | CC-BY |
| [Musopen](https://musopen.org) | Public-domain recordings and scores | Public domain (check each) |
| [Free Music Archive](https://freemusicarchive.org) | Wide range, filter by licence | Mixed |
| [OpenGameArt](https://opengameart.org) | Game music | Mixed |

Notes:
- Old tunes (shanties, hornpipes, jigs) are out of copyright, but a
  *recording* of one usually isn't. Use public-domain recordings, licensed
  tracks, or have the tune played or rendered fresh.
- Period instruments: fiddle, fife, drum, pennywhistle, hurdy-gurdy,
  voices. Avoid concertina and accordion: the concertina is 1830s, after
  our age of sail.
- AI music and sound generators (Suno, Udio, ElevenLabs) can be good;
  read their terms on commercial use before relying on them.

## Tools

| Tool | Use |
|---|---|
| [Blender](https://www.blender.org) | Modelling, conversion to glTF; scriptable, and can be driven by an agent through an MCP server |
| [Audacity](https://www.audacityteam.org) | Trimming, looping, and normalising sounds |
| [FluidSynth](https://www.fluidsynth.org) | Rendering MIDI music with a soundfont |
| [bevy_kira_audio](https://github.com/NiklasEi/bevy_kira_audio) | Better audio for Bevy: looping, crossfades, layering (check it supports our Bevy version) |

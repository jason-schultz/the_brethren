---
name: design-check
description: A short design brief before building a False Colours gameplay feature or rule change, and a soak against the balance targets after. Use before adding or changing any mechanic that touches ships, crews, combat, the economy, word of mouth, or factions, e.g. "/design-check leaks and pumps".
---

# Design check

A page of thinking before a gameplay change, and one soak after. It
catches what a feature does to the rest of the world: the player-only
rule that, applied to AI ships, made a raider fight the same merchant 71
times.

Keep it cheap: one brief, one soak, a short report. Don't loop on fixes;
report what's off and ask (the user watches their compute budget).

## Before building: the brief

Read `docs/DESIGN.md` (the pillars, and the section the change touches)
and `docs/BALANCE.md`, and the newest soak's summary
(`cargo logbook --summary`) if it's relevant. Then write the brief to the
user, under these headings, a few lines each:

1. **What the player should feel.** In play, not mechanics: "a battered
   ship racing for port is a story."
2. **The rule, simply.** One rule for every ship where it can be. Say
   plainly if it's player-only, and why.
3. **What it touches.** Go through each: AI captains (merchants, raiders,
   patrols, and their plans), the player, the economy and supply, word
   of mouth and standing, saves, the chronicle. For each AI role: does
   the rule change what its plans do next? Could two ships loop?
4. **What could go wrong.** Loops, ships stuck or stopped, things that
   never happen, things that happen every time, the player trapped with
   no way on.
5. **History and play.** What history says, and where play wins.
6. **The numbers.** Which knobs (`assets/data/tuning.ron`) it adds or
   uses, and which targets (`assets/data/balance.ron`) should move, and
   which way. Add a knob for any number someone may want to tune, and a
   target for any outcome worth watching, with hints for each.

Wait for the user's go-ahead if the brief changes what they asked for;
otherwise build.

## After building: one soak

1. `cargo soak --days 100`, then `cargo logbook --summary`.
2. Check the "Off the balance targets" list and the alarms against the
   brief's expectations: did the targets you named move the way you
   said? Did anything else move that shouldn't have?
3. Report in a few lines: what moved, what's off, and a suggested knob
   change if one is obvious. Don't tune it unasked; the user sets the
   targets and balances the numbers.

## Keep the docs true

- DESIGN.md: the feature's section, and the roadmap item.
- BALANCE.md: any new knob or target in its tables.

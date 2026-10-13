# The Brethren — Design Document

> A single-player, Age-of-Sail sandbox. You captain one ship, choose a side,
> and grow into a force on the world's oceans — or betray your side and
> become one.

This is a living document. When a design decision is made or changed, update
it here first, then build it.

---

## 1. Vision

An "Earth-bound EVE Online" under sail. The world is a political and economic
sandbox driven by three umbrella factions, each made up of many smaller
groups with their own goals. The player starts with a modest ship and a small
crew and makes a living through trade, hauling, privateering, naval service,
or outright piracy. Allegiance is a choice, not a class: players can defect,
be betrayed, lead mutinies, or take over the groups they belong to.

The title, **The Brethren**, fits whatever side you join: the Brethren of the
Coast for pirates, the brotherhood of a company's shareholders, or the band
of brothers in a navy crew.

### Pillars

1. **Sailing is the core.** Wind, points of sail, and seamanship decide
   travel and combat. If moving a ship around doesn't feel good, nothing else
   matters.
2. **Factions are made of people.** Umbrella factions are coalitions of
   guilds, companies, and squadrons. Their internal politics matter as much
   as the wars between them.
3. **Trade is a real career.** Hauling and trading should be a viable and
   interesting way to play. It is less deep than EVE's industry, but the
   whole economy runs on it.
4. **Loyalty is fragile.** Crews mutiny, captains defect, companies switch
   patrons, and pirate kings get deposed. The player can do all of these
   things, and so can the NPCs, to the player.
5. **Grounded in history.** Real Earth, real ports, real ship types, and a
   real-ish economy. We take creative license only where it makes the game
   better, and we write that license down (see §9).

---

## 2. Presentation & Controls

- **3D world, top-down camera.** Free pan/zoom over the ocean, with a strategy
  feel similar to EVE's overview. It is not a first-person helm.
- **Click to sail.** The player issues orders such as "sail to here",
  "follow", "engage", or "dock". The ship's crew carries them out under the
  constraints of wind and seamanship.
- **Pausable real time.** Space toggles pause. Orders can be issued while
  paused. Time acceleration is planned for long passages.
- **Single player, local only** (macOS first). There is no offline
  progression: the world only advances while the game is running and
  unpaused.

### 2.1 In Port (decided)

Docking switches to a **port screen**: an illustrated town view with
selectable locations (tavern, market, shipyard, governor's house,
merchants' exchange). This is the approach of *Sid Meier's Pirates!*, EVE
stations, and Mount & Blade towns. It is not first-person or a walkable 3D
town. Risky actions, such as sacking a strongbox or a night raid, become
short **scenes** (choices plus skill and crew checks) launched from a
location. A full tactical mode can come later if it earns its place.

Each location offers the jobs that fit who works there (§5.3): the
governor's house gives military and diplomatic work, the merchants'
exchange gives commercial hauls, and the tavern gives the work nobody
signs for.

---

## 3. Setting

- **Era:** roughly 1700–1730, the end of the Golden Age of Piracy. Trading
  companies are at their height, and the great powers are fighting over
  colonial trade.
- **Map:** the real Earth. The first playable region is the **Caribbean and
  the western Atlantic**. Later regions are West Africa, the Indian Ocean,
  and the East Indies (VOC heartland).
- **Scale:** world units are **meters**. Distances between ports will be
  compressed for gameplay (see §9). The ships themselves are true scale.

---

## 4. Factions

There are three **umbrella factions**. Each is a loose coalition of
**groups**. The player and every NPC captain have standing with individual
groups. Umbrella standing is derived from group standings and is never
stored separately.

### 4.1 Nations — loyal to the flag

The crowns: Great Britain, Spain, France, the Dutch Republic, Portugal, and
later others.

- **Groups:** squadrons, colonial governorships, admiralty boards, and
  privateer commissions (letters of marque).
- **Behavior:** expand influence, protect their shipping, and suppress
  piracy, unless the pirates are useful against a rival crown.
- **Cohesion:** high. Nations rarely fight themselves, but rivals fight each
  other constantly. Treason is rare and severely punished.
- **Player path:** commission → promotion → command of a squadron. Privateering
  is the gray zone that sits between Nations and Pirates.

### 4.2 Trading Companies — loyal to money

The VOC (Dutch East India Company), the English East India Company, the
French Compagnie des Indes, the Royal African Company, and smaller
chartered ventures.

- **Groups:** company chambers and factories (trading posts), convoy
  fleets, and shareholder blocs.
- **Behavior:** build trade routes and monopolies, and hire anyone useful,
  including navies, privateers, and pirates. They are the in-between faction.
- **Cohesion:** medium. Internal rivalry is about profit. A chamber may
  undercut another if the numbers work.
- **Player path:** contract hauler → company captain → factor or
  shareholder with a say in company policy.

### 4.3 Pirates — loyal to themselves

- **Groups:** pirate crews, the councils of pirate havens (Nassau, Tortuga,
  Port Royal before its fall), and the Brethren of the Coast. At the top are
  the **pirate lords**, who rarely unite except against an overwhelming
  threat such as a trading company crackdown.
- **Behavior:** raid, extort, and fence goods. They fight each other as
  readily as anyone else.
- **Cohesion:** low. Infighting is normal, and alliances last only as long
  as they pay.
- **Player path:** crewman's captain → crew of note → haven council → pirate
  lord.

### 4.4 Relationships

| | Nations | Companies | Pirates |
|---|---|---|---|
| **Nations** | rival crowns at war or uneasy peace | partners and tax base | hunt them, or hire them as privateers |
| **Companies** | buy protection and charters | compete for routes | hire, bribe, or crush them |
| **Pirates** | prey and predator | prey and occasional employer | each other's biggest threat |

### 4.5 Defection, Mutiny, Coups

- **Defection:** any captain, including the player, can change allegiance.
  The old side remembers.
- **Mutiny:** crew loyalty is tracked per ship. Low pay, poor food, losses,
  and harsh discipline erode it. Below a threshold, a mutiny can depose the
  captain. The player can *lead* mutinies on NPC ships they serve on, and can
  *suffer* them.
- **Coups:** groups have leaders. Members with enough influence can overthrow
  them, and the player can become a group's leader this way.

---

## 5. Core Systems (planned)

| System | Summary |
|---|---|
| **Sailing** | True wind, apparent wind, points of sail, and a per-hull speed polar. Ships cannot sail into the wind and must tack. Turning rate depends on speed. |
| **Navigation** | Orders (move, follow, dock, engage). The crew executes them with automatic tacking around the no-go zone. |
| **Combat** | Broadside arcs, reload time, shot types (round, chain, grape), and separate damage to hull, rigging, and crew. Boarding and capture. Holding the weather gage matters. |
| **Ports** | Docking, markets, shipyards, taverns (crew), and faction offices (contracts, commissions, fences). |
| **Economy** | Goods with per-port supply and demand. Prices respond to trade flows. Regional specialties include sugar, rum, tobacco, spice, cloth, and slaves. Slaves are handled as historical context, not as a player commodity; see §9. |
| **Crew** | Head count, skill, morale, and loyalty. Wages and shares. This drives mutiny. |
| **Standing** | Per-group reputation, with umbrella standing derived from it. Actions ripple through allies and enemies. |
| **Politics** | Group leadership, influence, coups, and alliances between groups. |
| **World AI** | NPC captains and groups pursue their own goals (trade, patrol, raid) while the game runs. |
| **Progression** | Money buys and outfits ships, from sloop to brig to frigate and beyond. Rank and influence come from standing. Fleets come later. |

### 5.1 Ships and Crew (proposed)

Every ship, the player's included, runs on the same rules.

- **Crew requirement.** Each hull has a *minimum crew* to work the ship
  day to day and a *full crew* to also fight her guns. Below the minimum,
  sail handling suffers: slower speed, slower turns, slower tacks. Below a
  skeleton crew, she can't make sail at all.
- **Berths.** Each hull has a maximum number of people aboard, crew and
  passengers combined.
- **Provisions.** Everyone aboard eats and drinks each day (food, water,
  rum). A crowded ship burns through stores fast. Running out hurts morale
  first, then lives. This also drives mutiny (§4.5).
- **Built-in asymmetry.** Historically, merchantmen sailed lean to save
  wages: a trading sloop carried around 10 to 15 men. A pirate sloop of the
  same size crammed in 70 to 100, which was enough to swarm a boarding but
  meant constant hunger for prizes and stores. Navies sat in between. The
  same rules produce this naturally: a big crew wins fights and starves; a
  small crew pays well and can't defend itself.

### 5.2 Cargo (proposed)

- **Hold capacity in tons** (burthen), per hull. Every good has a weight
  per unit, so a hold fills with ten tons of sugar or ten tons of sandstone
  equally. Valuable, light goods such as spice, silver, and indigo are what
  make a small, fast ship profitable.
- **Goods are data** (RON): name, weight, base value, category (food,
  luxury, building material, naval stores, munitions, and so on).
- **People are not cargo.** Soldiers, colonists, and diplomats take
  berths, not hold space, and eat provisions on the way.

### 5.3 Port Economy and Jobs (proposed)

One model drives the economy, the jobs, and port control. Jobs are not a
separate system: they are the economy's needs, shown to the player.

- **Every port has a profile** in its map data:
  - **Produces:** what it exports, grounded in what it really shipped.
    Sugar and rum from Jamaica, spice from the Malabar coast and the
    Moluccas, silver from Veracruz.
  - **Needs:** what it can't make for itself: provisions, timber, naval
    stores, manufactured goods, and soldiers.
- **Every port has a state:** supply, repair, garrison, and morale. These
  decay over time and are restored by deliveries. A port's state sets its
  market prices and its jobs.
- **Jobs come from needs.** A port short of timber posts a timber haul,
  sourced from a port that produces timber. A garrison running low asks for
  soldiers. A shaky governor asks for a diplomat to be carried in, which
  raises morale. A job is a pickup, a delivery, a deadline, a payment, and
  an issuer (a group, §4).
- **NPC ships fill the same needs.** Faction convoys and traders carry the
  same deliveries the player can. Whoever arrives first fills the need.
- **This is the reason for piracy and war.** A captured supply ship means a
  port goes without. A port that goes without long enough becomes weak:
  understrength, unrepaired, and demoralized. A weak port can be attacked
  and taken. Protecting a port means escorting its convoys and hunting its
  raiders. This is the "sovereignty" layer of an Earth-bound EVE.

#### Port Royal, c. 1715

| | Goods | Notes |
|---|---|---|
| **Produces** | sugar, rum, molasses, pimento (allspice), logwood, hides, ginger, indigo | Sugar dominated Jamaica by 1715. Jamaica was almost the only source of pimento. Logwood (a dyewood) was cut on the Central American coast and re-exported through Jamaica. |
| **Needs** | provisions (salt fish, flour, salt beef), timber, naval stores (tar, pitch, cordage, canvas), manufactured goods, munitions, soldiers | Provisions came from New England and Ireland because sugar land wasn't wasted on food. Naval stores were for the Royal Navy station. |
| **Also** | Spanish silver through contraband trade with the Spanish Main; a slave market | Silver is a high-value, light cargo, and a prime pirate target. The slave trade appears as context only (§9). |

Port Royal is the Royal Navy's Caribbean base in this era, so military work
is common there: munitions, naval stores, and dispatches.

---

## 6. Simulation Model

- **Simulation runs in `FixedUpdate`** on Bevy's virtual clock. Pausing
  stops the virtual clock, which stops the simulation. Rendering and input
  keep running.
- **Rendering and input run in `Update`** and never mutate simulation state
  directly. Input produces *orders*, and the simulation consumes them.
- **No offline progression.** If we go online later, it will be a major
  re-architecture, and we accept that.

---

## 7. Roadmap

### Milestone 1 — "Sailing feels good"

1. ✅ Project skeleton, docs, and coding standards.
2. ✅ Ocean, wind, one player ship, top-down camera, click-to-sail, and pause.
   Points-of-sail physics, so the ship can't sail into the wind.
3. ✅ Automatic tacking when ordered upwind. The ship beats inside a
   corridor around each leg, sized by how close that leg passes to land.
4. ✅ Port Royal and its approaches (Palisadoes, Port Royal Cays, Kingston
   Harbour) built from real coordinates, with land you can run aground on,
   route planning around land, and docking with a placeholder port screen.
   Next: a second port, and the real port screen (see §2.1).
5. One AI ship, plus broadside combat against it.

### Milestone 2 — "A living harbor"
1. The port screen with its locations (§2.1).
2. Goods data, a cargo hold, and a market. Port profiles with what each
   port produces and needs (§5.3).
3. Crew requirement, berths, and provisions (§5.1). Recruiting at the
   tavern.
4. Port state and jobs generated from port needs.
5. A second port, so there is somewhere to haul to.

### Milestone 3 — "Colors"
Factions and groups, standing, and NPC traders and patrols that react to the
player's allegiance.

### Milestone 4 — "Loyalty"
Crew morale and loyalty, mutiny, defection, and group leadership and coups.

### Later
Fleets, more regions (Africa, Indian Ocean, East Indies), weather and
storms, ship upgrades, save/load, and time acceleration.

---

## 8. Open Questions

- How much should distances between ports be compressed? This depends on how
  long a passage feels at 1× time.
- Fleet command: should it use direct control of escorts, or
  formation-level orders?
- Can the player lose their only ship? What does game over, or "start over
  as a common sailor", look like?
- Does time pass in port? For example, recruiting might take hours of game
  time, or the port screen could pause the world.
- Which second port? Candidates are Nassau (pirate haven), Havana or
  Cartagena (Spanish), and Bridgetown (English sugar island), or a closer
  town such as Spanish Town for a short first haul.

---

## 9. Creative License Log

Every deliberate deviation from history or physics goes here, with a reason.

| Deviation | Reason |
|---|---|
| Regional geography drawn at 0.5× real distance (`distance_scale` in each map file). Ships stay true scale | A crossing of the Port Royal approaches takes minutes rather than an hour. Open-ocean passages will need more compression, plus time acceleration. |
| Port Royal map is set c. 1715, after the 1692 earthquake and the 1703 fire | Keeps the town alive in our era while staying true to its decline. |
| Coastlines are hand-traced approximations | First pass. To be refined from real coastline data. |
| Port Royal trades all of Jamaica's goods. By 1715, much of the island's commerce had moved to Kingston | One port carries the region until Kingston is added. |
| Slave trade represented only as historical context (faction behavior, port descriptions), never as player cargo | Content choice. |

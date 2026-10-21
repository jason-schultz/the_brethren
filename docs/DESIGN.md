# False Colours — Design Document

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

The title, **False Colours**, fits whatever side you join. Every ship flies
colours, and a ship flying false ones hides who she is until she runs up her
true flag and opens fire: a real ruse of the age. The game's people do the
same: officers hide their loyalty, captains give false accounts, and the
player can hoist any nation's flag and change sides. (It was once called
The Brethren, which fit only the pirates and only men.)

### Pillars

1. **Sailing is the core.** Wind and seamanship decide travel and combat.
   If moving a ship around doesn't feel good, nothing else matters.
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
   better, and we write that license down (see §9). When realism and fun
   conflict, fun wins: this is a game, not a simulator.

---

## 2. Presentation & Controls

- **3D world, top-down camera.** Free pan and zoom over the ocean, with a
  strategy feel similar to EVE's overview. It is not a first-person helm.
  The camera follows the player's ship until the captain looks away: WASD
  or the arrow keys pan, dragging with the left button moves the map, and
  scrolling zooms toward the cursor. F returns to the ship.
- **Click to sail.** The player issues orders such as "sail to here",
  "follow", "engage", or "dock". The ship's crew carries them out under the
  constraints of wind and seamanship.
- **Pausable real time with time compression.** Space toggles pause, and
  minus and equals step the speed, or the top bar's buttons pick one:
  1×, 10×, 60×, 600×, and 5,400× (an hour and a
  half of game time every real second). Orders can be issued while paused.
  See §2.2.
- **Single player, local only** (macOS first, with builds for Windows
  and Linux too). There is no offline
  progression: the world only advances while the game is running and
  unpaused.

- **The HUD keeps to the edges.** A slim top bar has the date, the time
  buttons, heading and speed, wind, purse, crew and morale, days of
  stores, and warnings (paused, wanted, starving). Passing messages stack
  on the right and fade; a call that stops the clock stays pinned until
  the captain resumes. A battle box shows only while engaged.
- **Action bar and menu.** Along the bottom, an action bar (fire port,
  fire starboard, board, battle style, follow ship, on keys 1 to 5) and a
  menu of panels: Captain (C), Crew (R), Hold (H), Fleet (Tab), Nations
  (N), and Controls (F1), plus the influence overlay (M). Clicking a
  button and pressing its key are the same control. One panel is open at
  a time; its key or Escape closes it.
- **Spheres of influence.** M washes the sea in each nation's colour where
  it holds sway (England red, Spain gold, the Brethren bone white), with a
  legend at the bottom left. A nation holds the sea around its ports,
  farther the more loyal they are, and the lanes its ships keep sailing:
  wakes build where its ships pass, patrols counting triple, and fade by
  half every three days. Allies' and rivals' colours mix where they share
  the sea. Ship markers take their nation's colour.
- **Looking a ship over.** Ship names on the sea are in the colour of
  the flag each flies. Left-click a ship at sea for her card, at the top
  left: her name, rig, guns, colours, whether she's a merchantman, a
  warship, or a pirate, and her distance, heading, and speed. Her
  business can't be seen, only heard (§5.12), and the card gives what's
  been heard, where, how long ago, and what hangs on it. Escape or a
  click on open sea puts it away.
- **Rebindable keys.** The Controls panel rebinds any control's key;
  binding a taken key swaps the two. Bindings are kept in
  `keys.txt` beside the quicksave. WASD, the arrow keys, and Escape are
  fixed.
- **The captain's portrait.** The Captain panel shows him standing on the
  quay beside his ship, rendered live from simple shapes, above the ship's
  and captain's particulars. Real art can replace it later.

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
signs for. The town view lists everything the town needs, marking what's
short enough to post work for, and the market shows how much more of each
good it wants.

### 2.2 Long Passages (decided)

Long passages are fast-forwarded, not skipped. The world keeps running
underneath, so the sea is still where things happen.

- **Time compression.** At top speed the 30-hour passage from Port Royal to
  Nassau takes about 20 seconds.
- **Zoom out to the chart.** Scrolling out shows the whole sea, with ship
  markers sized to stay visible. Time speed never moves the camera. (It
  used to pull out to the chart automatically at 600×, which surprised
  players.) Ports can be clicked to dock from any zoom.
- **Alerts wake the captain.** Arriving or sighting a sail drops the game to
  1×. Ships lying in port aren't sighted. Anything that needs a decision
  drops it to 1× and pauses: running aground, provisions down to three
  days, or a contract due within a day. The HUD shows the call until the
  captain resumes or picks a speed.
- **The simulation holds up at speed.** The fixed step grows from 1/64 s up
  to 2 s at high speed, so 5,400× needs 2,700 steps a second instead of
  over 300,000. Tests sail the whole Port Royal to Nassau passage at the 2 s
  step.

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
stored separately. Built so far for nations and the player (§5.11): each
nation has a navy, a governor, and its merchants.

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

Built so far for nations (§5.7): every pair of nations has relations from
−100 to 100, moved by what their ships do to each other.

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
| **Sailing** | Ships sail straight on any heading. The wind is a speed modifier: a little slower into it, a little faster before it, set per hull. No tacking: realistic, but it made passages tedious. Turning rate depends on speed. |
| **Navigation** | Orders (move, follow, dock, engage). The crew steers each leg of a route planned around land. |
| **Combat** | Built (§5.5): broadside arcs, reload time, separate damage to hull, rigging, and crew, boarding, and prizes, in three battle styles. Later: shot types (round, chain, grape), and holding the weather gage. |
| **Ports** | Docking, markets, shipyards, taverns (crew), and faction offices (contracts, commissions, fences). |
| **Economy** | Goods with per-port supply and demand. Prices respond to trade flows. Regional specialties include sugar, rum, tobacco, spice, cloth, and slaves. Slaves are handled as historical context, not as a player commodity; see §9. |
| **Crew** | Head count, skill, morale, and loyalty. Wages and shares. This drives mutiny. |
| **Standing** | Per-group reputation, with umbrella standing derived from it. Actions ripple through allies and enemies. |
| **Politics** | Group leadership, influence, coups, and alliances between groups. |
| **World AI** | NPC captains and groups pursue their own goals (trade, patrol, raid) while the game runs. |
| **Progression** | Money buys ships (§5.1), from schooner and sloop to brig, fluyt, and frigate. Later: refits and upgrades. Rank and influence come from standing. Fleets come later. |

### 5.1 Ships and Crew (built)

Every ship, the player's included, runs on the same rules.

- **Crew requirement.** Each hull has a *minimum crew* to work the ship
  day to day and a *full crew* to also fight her guns. Below the minimum,
  sail handling suffers: slower speed and slower turns. Below a
  skeleton crew, she can't make sail at all.
- **Berths.** Each hull has a maximum number of people aboard, crew and
  passengers combined.
- **Provisions.** Everyone aboard eats 5 kg of provisions (food and
  water) a day. A crowded ship burns through stores fast. A starving crew
  loses a tenth of its men each day, and its morale collapses.
- **Morale** runs from 0 to 100, in five bands: mutinous (under 20),
  sullen, steady, content, and eager (80 and over). A new crew is steady.
  - It settles 4 points a day toward a resting level: 60 in port, 45 at
    sea (a quartermaster adds 10). A starving crew loses 15 a day instead.
  - Taking a prize lifts it 15, plundering 10, and an enemy striking or
    sinking 5. Being thrown back from a boarding costs 10, and losses cost
    in proportion: losing a quarter of the men costs 10.
  - A run ashore at a tavern costs 3 pieces of eight a man and lifts it 25.
  - Sullen and mutinous men work slowly (sailing and reloading at 90% and
    80%), and desert in port, a tenth a day. Eager men work at 110%.
  - Low morale is what makes a mutiny possible (§5.9).
- **Hiring.** Sailors sign on at the tavern for a bounty. Each port has its
  own pool of men, which refills over time. Nassau has plenty.
- **Ship classes are data** (`assets/data/ships.ron`). Every ship, the
  player's and the NPCs', is one of them:

  | Class | Rig | Guns | Hands / to fight / berths | Hold | Speed | Turn | Stoutness | Price |
  |---|---|---|---|---|---|---|---|---|
  | Schooner | two masts, fore-and-aft | 6 | 6 / 30 / 60 | 30 t | 6.0 m/s | 13°/s | 0.8 | 2,500 |
  | Sloop | one mast, gaff | 8 | 8 / 40 / 75 | 40 t | 5.5 m/s | 12°/s | 1.0 | 2,000 |
  | Brigantine | square fore, fore-and-aft main | 12 | 12 / 60 / 110 | 80 t | 5.3 m/s | 10°/s | 1.4 | 4,500 |
  | Brig | two masts, square | 14 | 16 / 80 / 130 | 110 t | 5.1 m/s | 9°/s | 1.8 | 6,500 |
  | Fluyt | three masts, square, built to carry | 10 | 14 / 50 / 90 | 250 t | 4.4 m/s | 7°/s | 2.0 | 7,000 |
  | Frigate | three masts, square, a sixth rate | 24 | 40 / 160 / 220 | 120 t | 5.4 m/s | 8°/s | 3.0 | 18,000 |
  | Longboat | lugsail | 0 | 2 / 8 / 24 | 4 t | 4.5 m/s | 20°/s | 0.3 | (not sold) |

  Each is a trade-off: the schooner is the fastest and handiest but
  carries little and fights little; the fluyt carries six times a sloop's
  cargo and can't run from anyone; the frigate outguns everything and eats
  like a town. Fore-and-aft rigs lose less into the wind, square rigs gain
  more before it. *Stoutness* is how much shot she takes, as a multiple of
  a sloop: a hit does a sloop's damage divided by it. The longboat is what
  a captain is left with after a mutiny (§5.9).
- **Changing ships.** Every shipyard sells every class but the longboat.
  Your ship goes in part exchange, at what she'd fetch as a prize (§5.5)
  without her cargo, and the new one comes sound and fully armed. Your
  crew, cargo, officers, prizes, and record come with you; the yard won't
  sell you a ship without berths for your crew or room for your cargo. She
  keeps your ship's name.
- **The player starts in a sloop** with 12 men, 2 tons of provisions
  (about a month), and 500 pieces of eight.
- **Built-in asymmetry.** Historically, merchantmen sailed lean to save
  wages: a trading sloop carried around 10 to 15 men. A pirate sloop of the
  same size crammed in 70 to 100, which was enough to swarm a boarding but
  meant constant hunger for prizes and stores. Navies sat in between. The
  same rules produce this naturally: a big crew wins fights and starves; a
  small crew pays well and can't defend itself.

### 5.2 Cargo (built)

- **Hold capacity in tons** (burthen), per hull. Freight is traded by the
  ton, so valuable goods such as indigo and pimento are what make a small,
  fast ship profitable.
- **Goods are data** (`assets/data/goods.ron`): name, base price, and
  carriage. Prices are in pieces of eight, the common coin of the West
  Indies.
- **People are not cargo.** Soldiers, and later colonists and diplomats,
  take berths, not hold space, and eat provisions on the way.
- **Owned and consigned.** The hold keeps the captain's own goods apart
  from goods carried under contract. A contract is paper; the cargo is
  physically aboard. Later, a captured ship's consignments go with her.

### 5.3 Port Economy and Jobs (built)

One model drives the economy, the jobs, and port control. Jobs are not a
separate system: they are the economy's needs, shown to the player.

- **Every port has a profile** in its map data:
  - **Produces:** what it exports, grounded in what it really shipped.
    Sugar and rum from Jamaica, spice from the Malabar coast and the
    Moluccas, silver from Veracruz.
  - **Needs:** what it can't make for itself: provisions, timber, naval
    stores, manufactured goods, and soldiers.
- **A port's state is its stock.** Each good a port deals in has a stock,
  a target, and a daily rate: produced goods build up, and needed goods run
  down. Prices rise as stock falls below target and fall as it piles up.
  Supply and repair are the stocks of provisions, timber, and naval stores.
  The garrison is the stock of soldiers. Morale, and the diplomats that
  restore it, come with politics (Milestone 3).
- **Markets** sell and buy at the price plus or minus 10%, one unit at a
  time, so a big purchase drives the price up as it goes.
- **Jobs come from needs.** When a port's stock of a needed good falls
  below half its target, it posts a job at the nearest port that produces
  that good: up to 30 tons or heads, a fee by distance, and a deadline of
  twice a slow merchantman's passage plus a day. The producer hands over the
  goods on acceptance. Delivering them restocks the port in need; late
  delivery pays half, and offers nobody took are withdrawn at the deadline.
  Freight work is offered at the merchants' exchange and soldiers at the
  governor's house, or at the tavern where a port has neither. Jobs will
  gain an issuer (a group, §4) with factions.
- **NPC ships fill the same needs.** Faction convoys and traders carry the
  same deliveries the player can. Whoever arrives first fills the need.
- **No trade with the enemy.** A port never posts a job at a port of a
  nation it's at war with.
- **This is the reason for piracy and war.** A captured supply ship means a
  port goes without. A port that goes without long enough turns against
  its masters and can be lost to them (§5.10). Protecting a port means
  escorting its convoys and hunting its raiders. This is the
  "sovereignty" layer of an Earth-bound EVE.

#### The first ports, c. 1716

| Port | Produces | Needs | Notes |
|---|---|---|---|
| **Port Royal** (England) | soldiers, sailors | provisions, naval stores, munitions, rum, timber | The Royal Navy's Caribbean base. It eats stores and sends out soldiers. |
| **Kingston** (England) | sugar, rum, molasses, pimento, ginger, indigo, hides, logwood; and, shipped in, provisions, manufactures, naval stores, munitions, timber | soldiers, salt | Jamaica's commercial port. Sugar dominated the island; Jamaica was almost the only source of pimento; logwood was cut in Central America and re-exported through Jamaica. Provisions came from New England and Ireland because sugar land wasn't wasted on food. |
| **Nassau** (pirates) | salt, sailors | provisions, rum, munitions, naval stores | The Republic of Pirates, 1716 to 1718. No governor and no exchange: business is done in the taverns. |
| **Santiago de Cuba** (Spain) | hides, provisions (salt beef), timber, soldiers, sailors | manufactures, munitions, naval stores, salt | Spain's nearest port to Jamaica. Its own fleets come seldom, so it buys Jamaican manufactures, smuggled or not, and its guarda costas hunt the smugglers. |

Spanish silver, smuggled in from the Spanish Main, is left out for now:
silver is money, so it needs its own treatment as plunder rather than as
freight. The slave trade appears as context only (§9).

### 5.4 Merchant Captains (built)

Seven NPC merchantmen work the same contracts the player does, each
with half her guns mounted and two hands more than she needs, through the same orders: accept a job, buy provisions,
cast off, sail to dock. Five sail under English colours from Jamaica
(the sloops Swallow and Two Brothers, the brigantine Mary Ann, the fluyt
Endeavour, and the schooner Dove), and two under Spanish colours from
Santiago (the fluyt Santa Rosa and the sloop San José). Lost
ones are replaced at a port of their nation.

- **In port.** A merchant lies in port for 6 hours, then chooses: a
  contract already aboard comes first, then the best-paying job she can
  take here. With no work here, she sails to the port with the most work
  waiting. With none anywhere, she looks again in an hour.
- **Loading.** Half an hour after choosing, she casts off, or two hours
  if she's asked the player for an escort (§5.12). The opening
  fleet sails at 10-minute intervals, so there are ships at sea in the
  first hour of a new game.
- **On the map.** Ships at sea carry their names. Right-clicking one lying
  in port explains she must be caught at sea.
- **Competition.** Merchants take jobs the player might have wanted, and
  they restock the ports in need. That keeps prices and jobs moving.
- **Sightings.** A merchant at sea within sight drops the game to 1×. Ships
  lying in port aren't sighted.

### 5.5 Battle (built)

Right-click a ship at sea to engage her. Anyone can be attacked; standing
and reputation come with factions.

- **Battle styles.** The battle style control (4) cycles them, and the
  action bar shows which is active:
  - *Auto:* your ship lays herself alongside, and each broadside fires as
    it bears and is loaded.
  - *Manual:* you steer (right-click to sail as usual), and the fire
    controls (1 and 2) fire the port and starboard broadsides.
  - *Abstract:* the world pauses and the battle is fought by rounds on a
    screen: close in, broadside, rake her rigging, board, or break off.
    When one ship closes and the other opens the range, the faster is
    likelier to win. Dice vary the damage.
- **Gunnery.** A broadside bears within 35° of abeam, out to 400 m, and
  reloads in 30 s. Hits fall off with range. Each hit costs timbers,
  rigging, and men; a stouter ship loses less timber and rigging to each
  (§5.1).
- **Damage.** Rigging damage slows a ship. Crew losses weaken sail
  handling and boarding. Timbers at zero sink her.
- **The enemy.** An attacked captain sizes you up (guns, stoutness,
  damage, and hands): if she's at least as strong, or you're badly battered, she fights broadside-on. Otherwise she
  runs, downwind if that opens the distance, firing as her guns bear.
  Beaten (timbers, rigging, or crew), she strikes her colours.
- **Speed.** A full hold is up to 25% slower, so a raider sailing light
  catches a laden merchantman. One in ballast may get away.
- **Boarding.** Within 60 m, the board control (3) boards. A struck ship is carried without a
  fight. Otherwise crews fight it out, and defenders count for 1.25 men
  each. Morale and a first mate make each man count for more or less. The victor takes her purse and as much cargo as fits; her
  contracts are void.
- **Prizes.** Two spare hands (beyond the least that can sail your ship) sail her as a
  prize: a prize crew works her home under easy sail, so two sail her as
  well as a full crew. She
  follows you and docks where you dock. At a shipyard you can sell her,
  or take command of her and leave your old ship as the prize. With no
  hands to spare, she's plundered and let go.
- **Shipyards** repair timbers and rigging for a fee, in proportion to
  her price, and sell new ships (§5.1).
- **Losing.** Your ship sinking ends the game until you load a save.
  Losing her to mutiny or the law doesn't (§5.9).
- **Replacements.** A merchant sunk or taken is replaced two days later.

### 5.6 Fleet (built)

The Fleet panel (Tab) shows the flagship and her prizes, with where each
lies, her crew, hold, and damage, and a Look button to find her.

- **Orders.** Each prize can be told to follow you (the default), wait
  where she is, or sail to a port and lie there.
- **Transfers.** A prize lying alongside (within 200 m at sea) or in the
  same port can pass cargo and men to and from the flagship, in lots of 1
  and 10, within hold room and berths.

### 5.7 Nations and Standing (built)

- **Colours.** Ships and ports fly the colours of England, Spain, or the
  Brethren (the pirates). Port flags come from the map data, and change
  when a port goes over (§5.10). The player starts under English colours
  and can hoist any nation's from the Captain panel, except in action.
  Her colours decide whom her deeds and deliveries count for: ports
  favour them, enemy ports turn them away, and hostile navies and raiders
  hunt them. Her standing is her own, whatever she flies.
- **Standing** is kept with each of a nation's groups (§5.11), from −100
  to 100; your standing with the nation is the mean of its groups'. You
  start at +10 with every English group. Attacking a nation's ship costs
  15, taking her 20, and sinking her 25: in full with her navy if she's a
  warship, or her merchants if not, and half that with the nation's other
  groups. Every blow against a nation earns a third as much with the
  navies of its enemies (hostile or at war), and costs a third with its
  allies'; that's how the Brethren come to admire a captain who raids the
  crowns. Delivering a contract earns 3 with the port's merchants for
  freight, or its governor for soldiers. Seeing an escorted merchant in
  earns 5 with her merchants, and losing her costs 5. Each nation's share
  moves only when word of the deed reaches it (§5.12). Bad standing fades
  by 1 a day; good standing stays.
- **Wanted.** At −30 or below you're wanted: that nation's ports turn you
  away, and its navy hunts you. One prize is a warning; a second makes you
  wanted.
- **Relations between nations.** Every pair of nations stands from −100
  to 100: at war at −50 or below, hostile below −25, allies at 50 and
  up, and at peace between. In 1716 England and Spain are at an uneasy
  peace (−20), the Brethren are hostile to England (−30), and at war
  with Spain (−60). Any ship's deeds under colours move relations
  between her colours and the nation she touched: attacking −2, taking
  −3, sinking −4, delivering to another nation's port +1, and lying in a
  port as it rises against its nation −25. Nations already at war take
  no further offence. Relations drift back to their 1716 footing by 0.5
  a day. Changes of terms are news in the HUD.
- **What war means.** Navies hunt the ships of nations they're at war
  with, a captain their nation wants, and anyone they see attacking their
  own or an ally's shipping. Ports turn away the player under an enemy's
  colours, unless the town deals with that nation all the same (favour
  50 or more), and no port posts a job at an enemy's.
- **Patrols.** Two English men-of-war (the brig HMS Swift and the sloop
  HMS Ferret) and a Spanish guarda costa (the brigantine San Miguel),
  each fully manned with every gun mounted, cruise
  every port but their enemies' and victual at their own nation's. One
  that sights her quarry gives chase and engages within 1.8 km. Lost
  patrols are replaced at a home port after four days.
- **Pirate raiders.** Three ships of the Brethren (the sloops Ranger and
  Revenge and the brigantine Adventure; every gun mounted, and three
  quarters of a fighting crew) fit out at pirate ports. Each carouses a day
  in port, then lies in wait halfway along a lane between two ports picked
  at random. She preys on any ship but a warship whose colours are
  hostile to hers, the player's included, and goes home after four days,
  or once plunder fills half her hold, to sell it at the nearest port
  that will have her. Lost raiders are replaced after three days.
- **NPC prizes.** NPC captains board whoever strikes to them, plunder her
  purse and cargo, and burn her. Her contracts are lost with her, so the
  port that needed them goes without.
- **Sea room.** Ships steering by battle tactics turn aside from land
  within 600 m ahead.

### 5.8 Saving (built)

A new game opens from a save, too: `assets/data/opening.ron`, the world
18 hours after it began, so every merchant, patrol, and raider is already
at sea about her business when the player takes command in Port Royal. A
test grows it from the fresh world (every fleet in port) and writes it
again when the map, the fleets, or the save format change.

Quicksave (F5) and quickload (F9) go to one RON file in the player's
folder for application data: `~/Library/Application Support/False Colours/`
on macOS, `%APPDATA%\False Colours\` on Windows, and
`~/.local/share/false-colours/` on Linux. The key bindings live there too.
A folder left under the game's old name, The Brethren, is moved there the
first time the game or its tools look. The map, goods, and ship
classes are built into the game, so a save holds only what changes: the
clock, port stocks, ships with their crews' morale (and each captain's
standing, service, and what she's heard), contracts, escorts, officers,
and word of sailings, referring
to each other by name.
Ships under way are given their voyage again as an ordinary sail order. A
save that names anything the game doesn't have is refused without changing
anything. Loading returns to 1× with the camera on the player's ship.

### 5.9 Officers and Loyalty (built)

Officers hold one post each, and bring one plain advantage while they're
free to do their duty:

| Post | Advantage |
|---|---|
| First Mate | Each boarder counts for 1.25 |
| Sailing Master | 10% more from her sails |
| Gunner | Reloads in three-quarters of the time |
| Bosun | Repairs rigging (10% a day) and timbers (5% a day, up to 75%) at sea |
| Surgeon | Saves 30% of the wounded |
| Quartermaster | Resting morale 10 higher |

- **Getting them.** Every tavern has two officers looking for a post, for
  a bounty of 100 to 250 pieces of eight; they move on after a few days.
  A ship taken or plundered gives up her mate as a prisoner, who can be
  signed on for nothing. One officer per post.
- **Hidden loyalty.** Each officer is loyal, ambitious, or a turncoat, and
  the captain never sees which. Hired in an English port: 60%, 25%, 15%.
  In Nassau: 40%, 45%, 15%. Prisoners: 35%, 30%, 35%.
- **Signs.** The ambitious are heard grumbling with the hands once the men
  are less than content. Turncoats are seen ashore with the harbourmaster's
  clerk in lawful ports, and write letters they show no one at sea. Loyal
  officers now and then look just as suspicious.
- **Dealing with doubts.** An officer can be put in irons (no duty, and no
  mischief), released, put ashore in port, or marooned at sea.
- **Mutiny.** Below morale 30 at sea, an ambitious officer leads a rising,
  about an even chance a day; below 10 the hands rise without one. The
  share of the men who follow grows as morale falls (half at 25, all at
  0). Against them stand the rest of the hands, the captain, and each
  loyal officer, worth two. Put down, half the mutineers are lost, the
  ringleader goes in irons, and morale settles at 40.
- **Betrayal.** Docking at a port of a nation you've crossed (standing
  below 0, but not wanted), an unconfined turncoat has an even chance of
  selling you out. Soldiers are waiting on the quay.
- **Losing your ship.** Mutineers sail off with her and her purse, bar the
  fifth in your coat; you're set adrift in a longboat with the men who
  stood by you. Arrested, the ship and the prizes lying there are seized
  and you pay half your purse in fines, and two hands wait for you with a
  boat. Either way your prizes elsewhere and your loyal officers follow
  you, your contracts are lost, and word gets around: −10 standing with
  every nation. The longboat (no guns, 4 tons, 24 berths) can still carry
  small contracts, hire men, and board a weak merchantman, and at a
  shipyard you can take command of any prize.

### 5.10 Ports and Allegiance (built)

Every port regards each nation with a favour from 0 to 100.

- **Settling.** Favour settles toward 55 for the nation that holds the
  port and 25 for the others, 5% of the way a day.
- **Neglect.** While a port is short of the things it needs (below half
  its target), its owner loses up to 2.5 favour a day, in proportion to
  the share of its needs that are short. A port short of everything sinks
  to about 5; one short of a few things grumbles but holds.
- **Supply.** Whoever brings a port what it needs, by contract or at
  market, wins favour for the colours she flies: 25 for filling the whole
  gap below target of one good, in proportion. The other nations lose
  half that much. Only the gap counts, so flooding a full market wins
  nothing. This is how pirates selling plunder win over a town, and how
  a nation that lets a port go hungry loses it.
- **Restless.** Below 25 for its owner, a port is restless: news in the
  HUD, and its garrison deserts at two men a day.
- **Going over.** A restless port rises for the nation it favours most,
  if more than its owner, when a ship under that nation's colours lies in
  its harbour and none of its owner's warships stand within 1.5 km. The
  daily chance is 40% with no garrison, less as its soldiers near their
  target, and none at full strength. The port takes its new flag with 60
  favour for its new nation and 10 for its old one. The ship that was
  there is credited: −25 relations between the nations, and if she's the
  player, −15 standing with the old nation (+5 with the Brethren).
- **Dealing with the enemy.** A town with 50 favour for a nation lets
  its ships in even at war with its owner.
- **Seen in port.** The town overview gives the port's mood and its
  favour for every nation; the Nations panel shows every port's, and the
  terms between every pair of nations.

### 5.11 Groups, Service, and Pardons (built)

Each nation is three groups, and the player has standing with each:

| | Navy | Governor | Merchants |
|---|---|---|---|
| **England** | the Royal Navy | the Governor of Jamaica | the Jamaica merchants |
| **Spain** | the Armada de Barlovento | the Governor of Cuba | the Cuban merchants |
| **The Brethren** | the Flying Gang (the raiders) | the Nassau council | the fences of Nassau |

- **Letters of introduction.** A new captain finds three letters waiting
  on the Captain panel, each with a group's reasons to join it and an
  errand to run: the Governor of Jamaica's soldiers from Port Royal to
  Kingston, the Cuban merchants' manufactures from Kingston to Santiago,
  and the Flying Gang's powder and shot from Kingston to Nassau. A foreign
  group's agent waits in the tavern. Errands are contracts only she can
  take, paid as usual, and running one raises her 20 with that group (half
  that with the rest of its nation): enough to join. She can run all three
  and then choose, at no cost with the others.
- **Joining.** A group takes a captain at +20 with it, at its office in a
  port its nation holds: the navy and the governor at the Governor's
  House, the merchants at the Merchants' Exchange, or the tavern in a port
  without one. A crown's navy takes only captains flying its colours. You
  serve one nation at a time, in as many of its groups as will have you;
  resign from the Captain panel to serve another.
- **Rank** rises with standing, at +20, +50, and +80, and is never lost
  short of being struck off. Promotions are news.

  | | 1st | 2nd | 3rd |
  |---|---|---|---|
  | Navy | Lieutenant | Captain | Commodore |
  | Governor | Privateer | Commissioner | Councillor |
  | Merchants | Associate | Partner | Alderman |
  | Flying Gang | Gentleman of Fortune | Captain of Note | Pirate Lord |
  | Nassau council | Freebooter | Councillor | Elder |
  | Fences | Runner | Partner | Master Fence |

- **What service brings.** The navy pays 5, 10, or 20 pieces of eight a
  day by rank. The governor's letter of marque pays 100, 200, or 400 for
  every ship of a nation at war with yours taken or sunk under your own
  colours. The merchants pay 10%, 20%, or 40% more for their contracts.
  Every group sends word of the sailings that concern it (§5.12).
- **Struck off.** A blow against your own nation's ships or ports strikes
  you off every one of its groups, and so does being wanted by it. A
  crown's navy officer who hoists other colours is struck off the navy as
  a deserter. The Brethren fly what they like.
- **Acts of Grace.** England proclaims one 45 days in, and Spain 105, and
  each again every 120 days, for 30 days at a time. While one is in force,
  a captain the crown wants may come into any of its ports with a
  governor, whatever colours she flies, and is pardoned as she docks,
  unless she's notorious (−80 or worse). The Nations panel gives the dates.
- **Petitions.** A lawyer in any tavern will carry a petition to a crown
  that wants you, for 10 pieces of eight for every point your standing
  with each of its groups is below −10. It's granted at once.
- **Pardoned.** Either way, your standing with each of the crown's groups
  rises to at least −10, and you're no longer wanted. The Brethren pardon
  no one; they only forget. Short of a pardon, fighting a crown's enemies
  wins back standing with its navy.

### 5.12 Word of Mouth and Escorts (built)

A ship's flag, rig, and guns can be seen; her business can only be heard.

- **The ledger.** Everything worth talking of is news: a ship sailing,
  a captain's deed, a wreck. The world keeps the truth of each, where it
  happened, and the ports where it's known. Sailings go stale after 4
  days, other news after 30.
- **How word travels.** A thing done in port is known there at once. A
  thing done at sea is known only to those who saw it: ships within sight
  (8 km), the ship it was done to if she's still free, and a taken ship's
  crew, set adrift to row for the nearest port at a meter a second. A
  ship that docks tells her port all she carries, and a ship that sails
  carries everything known in her port to the next, so news travels the
  trade lanes as fast as the shipping. A sailing is known at once where
  she sailed from and where she's bound. A captured ship's old voyage is
  forgotten; her taking is the news now, and it reaches port with her, a
  witness, or her crew in the boats.
- **Overdue.** A merchant who sails with a contract is due where she's
  bound when a contract would be: twice a slow merchantman's passage
  along the chart, and a day. If she hasn't come in by then, she's posted
  overdue there, and the talk names every ship that was within 30 km
  when she was lost, if she was: the one who did it among them, and any
  innocent passing by. With no one about, no one knows what became of
  her. It's only talk: suspicion alone moves no one's standing.
- **One deed, not three hundred.** Closing again on a ship already
  attacked, after a chase broke off, is the same attack; taking again a
  ship plundered and let go is the same taking. Neither counts anew while
  the first is still news. And a fight is one deed however far it goes:
  a nation that heard of the attack and then hears she was taken answers
  only for the rest (the taking's worth, less the attack's), and one
  that hears of the taking first takes no notice of the attack. A ship
  taken and then sunk pays a privateer's bounty once.
- **Accounts.** A captain lying in port with a tavern can give an
  account of any loss she was within 30 km of: the ship was wrecked, or
  taken or sunk by one of the other ships about. Never by herself; she
  needn't confess. It's news like any other, starting in that port, and
  it's taken at her word, true or not: by itself it moves no one's
  standing. A false account is found out in any port where it's known
  beside word of the loss itself (the deed of her taking or sinking, or
  her wreck, from a witness or the survivors), and that's a deed of its
  own: −30 with the lost ship's group, and the account is no longer
  talked of. Sink a ship unseen and blame the reef, and no one will ever
  know; take her and blame the Ranger, and her crew in the boats will
  tell otherwise. A ship lost before the game was loaded can't be
  accounted for.
- **Reputation by word.** A deed's worth is weighed when it's done, but
  a nation acts on it only when it hears: when the news is known in one
  of its ports. Only then does the captain's standing with its groups
  move, relations between it and her colours change, its groups strike
  her off or pay her bounty, and she hears that word has got out. Sink a
  ship with no one in sight, and no one knows. Take her, and her crew
  will tell.
- **Tavern talk.** Lying in a port with a tavern, a captain hears what
  it talks of, and the tavern lists it in sentences: sailings, deeds,
  wrecks. Each captain hears her own version. Talk of a sailing may get
  the cargo wrong (another good that travels the same way). It's true 60% of the time for a captain
  who stands at 0 with the port's merchants, plus 0.4% for every point of
  standing, between 40% and 95%.
- **Intelligence.** The groups the player serves send her the truth of
  the sailings that concern them, as news: a crown's navy, raiders of
  nations hostile to it putting to sea; a governor, foreign soldiers on
  the move; a crown's merchants, their own nation's cargoes; the Flying
  Gang and the fences, the shipping of nations hostile to the Brethren.
- **What hangs on it.** The ship card says what her voyage means, from the
  same rules that decide it (§5.10): how much of the port's shortage her
  cargo fills, the favour that wins her colours there, whether the port
  stays short (and its owner goes on losing favour) if she's lost, and for
  soldiers bound for a restless port, its chance a day of rising now and
  once they land. A raider's card names the shipping she endangers.
  Nothing is said that the world doesn't do.
- **Escorts.** A merchant about to sail with a contract, from a port
  where the player lies, asks the merchants' office for an escort if the
  Brethren's raiders would prey on her colours. She waits two hours for
  an answer. The fee is half her contract's (at least 20 pieces of
  eight). Once taken, she sails when the player does, waiting up to 12
  hours past her time, and the player keeps company with her: her ship
  sails in the ward's wake, closing to 120 m astern whenever she falls
  more than 250 m behind (the same rule a prize follows her captor by).
  Any other order ends it; right-clicking the ward takes it up again
  rather than engaging her. In action, the player fights first and
  resumes company after. If the ward is attacked, the game pauses with a
  call naming her attacker. If she comes into her destination with the player
  within sight (8 km), the fee is paid and her merchants think 5 better
  of the player. If she's lost, burned by raiders or taken by anyone, the
  player included, the contract fails. Her merchants then wait 3 days
  for the escort's account of it; given one, true or not, they think no
  worse of her, and without one it costs 5. Coming in without her escort
  in sight pays nothing.
- **Wrecks.** Every ship that goes down leaves word of it, and within
  5 km of land or a shoal, a wreck with her hold and strongbox. Ships
  sink in battle (no survivors), are burned after they're plundered, or
  are lost running aground (faster than 1.5 m/s costs 15% of her timbers
  for every meter a second over) or striking a reef (3% a day within
  2 km of a shoal, checked each hour, costing 20% to 70% of her timbers).
  Those wrecked on the shore or a reef row for port and tell. The player
  wrecked takes to the boat with her men, as after a mutiny. The sea
  takes 2% of a wreck's cargo a day and breaks her up after 30 days.
- **Salvage.** Any ship hove to within 150 m of a wreck brings up 0.05
  tons an hour a man, into her hold: the strongbox first, then the
  dearest cargo by the ton; people drowned stay down. Salvaging another
  nation's wreck is a deed against its governor (−5). Wrecks show on the
  chart once heard of or in sight; right-click one to lie over her, and
  left-click for her card.
- **Everyone comes.** A raider that hears of a wreck in port dives on it
  before lying in wait on a lane, and preys on whoever else comes. A
  merchant with no contract goes out diving, and sells what she brings
  up at the next port. When a nation hears of its own ship wrecked, its
  nearest patrol stands guard, salvaging for her home port, and hunts
  anyone else diving on her.
- **Sightings say who's coming.** "Sail ho!" names what the ship is (a
  merchant sloop, a sloop of war, a pirate sloop), and warns "She'll come
  for us!" when she's a warship or raider whose rules (§5.7) would have
  her hunt the player.

---

### 5.13 The Chronicle, Soaks, and the Logbook (built)

Tools for playtesting, not part of play.

- **The chronicle** writes what happens in a run as it happens, one JSON
  record a line, in the logs folder beside the quicksave: news and how
  fast word of it reaches each nation, every battle with each side's class,
  crew, and damage at its start and end, the player's standing as it
  moves and why, her money, orders, controls, and alerts, nations coming
  to new terms, ports going over, and a snapshot of the world once a game
  day (relations, ships by role, every port's prices, memory). It's only
  in builds with the `chronicle` feature (`dev` has it). The last 20 runs
  are kept, up to 200 MB.
- **Notes.** F8 pauses and opens a box to type a note in, kept with a
  snapshot of that moment.
- **Alarms** are raised once each when something looks broken: a ship with
  a route that hasn't moved in a day, a position that isn't a number, a
  port with negative stock or a month without a good it needs, a war two
  months long, the player's standing pinned at ±100.
- **Crash reports**, in every build: a panic writes what panicked, the
  backtrace, and the run's last 300 records, with a copy of the quicksave,
  to the crashes folder. Warnings and errors the game logs are kept too.
- **Soaks.** `cargo soak` runs a new game's world headless with no one at
  the helm, as fast as it goes (100 days in about a minute), chronicled.
- **The logbook.** `cargo logbook` serves the logs as web pages on this
  machine: every run, each in full with charts, two compared, the trend
  of headline numbers over the last 20 runs of a kind, and crash reports.
  Each run's summary is plain text to paste into a conversation, leading
  with alarms, notes, and what changed since the run before.

## 6. Simulation Model

- **Factions are data.** `assets/data/factions.ron` lists every nation:
  its name, colours, whether it's lawful (a crown, with pardons and
  patrols) or not (raiders, no pardons), its Acts of Grace, its three
  groups with their names, rank titles and pitches, its officers' loyalty
  odds, the player's standing with it at the start, and its fleets by
  name. It also holds how the nations stand at the start and what every
  deed is worth. Adding a nation is adding an entry; no code names one.
  The game checks the file as it loads (unique keys, known nations in the
  relations, a worth for every deed, a title for every rank).
- **A captain's record goes with her.** Standing, the groups she serves,
  and what she's heard are components of the ship she commands, not
  global state, and move with her when she loses a ship or takes command
  of a prize. A deed moves the standing of the captain who did it;
  hunters, ports, and service judge each captain by her own record. Only
  the player has a record for now, but nothing assumes there's one. A
  captain's id stays with her from ship to ship, so the news of her deeds
  finds her wherever she's moved.

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
3. ✅ Wind as a speed modifier. (Tacking and a speed polar were built
   first, then dropped because they made passages too slow to be fun.)
4. ✅ Port Royal and its approaches (Palisadoes, Port Royal Cays, Kingston
   Harbour) built from real coordinates, with land you can run aground on,
   route planning around land, and docking.
5. ✅ Battle against the merchant fleet: three battle styles, boarding,
   prizes, and repairs (§5.5).

### Milestone 2 — "A living harbor"
1. ✅ The port screen with its locations (§2.1).
2. ✅ Goods data, a cargo hold, and a market. Port profiles with what each
   port produces and needs (§5.3).
3. ✅ Crew requirement, berths, and provisions (§5.1). Recruiting at the
   tavern.
4. ✅ Port stock and jobs generated from port needs.
5. ✅ More ports: Kingston across the harbour, and Nassau, on a West Indies
   map with Cuba, Hispaniola, and the Bahamas. A world calendar starting 1
   June 1716.
6. ✅ Time compression, zooming out to the chart, and alerts (§2.2).

### Milestone 3 — "Colors"
1. ✅ NPC merchant captains carrying the ports' contracts (§5.4).
2. ✅ Quicksave and quickload (§5.8).
3. ✅ Nations, standing, wanted status, and navy patrols (§5.7). The fleet
   screen, prize orders, and transfers (§5.6).
4. ✅ Spanish ports and ships, pirate raiders, relations between nations,
   ports' allegiance and changing hands, and the influence overlay (§5.7,
   §5.10).
5. ✅ Groups within nations, service and rank, and pardons (§5.11).
6. ✅ Ship cards, word of mouth, intelligence, and escorts (§5.12).
7. ✅ The ledger of news, reputation by word, wrecks, and salvage (§5.12).

### Milestone 4 — "Loyalty"
1. ✅ The HUD at the screen's edges, the action bar and menu, panels, and
   rebindable keys (§2).
2. ✅ Crew morale (§5.1).
3. ✅ Officers, hidden loyalty, mutiny, and betrayal (§5.9).
4. Defection, and group leadership and coups.

### Milestone 5 — "Bigger ships"
1. ✅ Ship classes as data, NPC fleets of mixed classes, stoutness in
   battle, buying ships at the shipyard, and models rigged by class (§5.1).
2. Refits and upgrades: a few per hull, each one fixed advantage (copper
   sheathing, more guns pierced, a bigger hold, more berths). Shot types.
3. Better ship models and art.

### Later
Fleets, more regions (Africa, Indian Ocean, East Indies), weather and
storms, and multiple save slots.

---

## 8. Open Questions

- Now that time compression exists, should the map go back to real
  distances (`distance_scale` 1.0)? Passages would double in length, about
  two minutes at top speed for Port Royal to Nassau.
- Fleet command: should it use direct control of escorts, or
  formation-level orders?
- Can the player lose their only ship? What does game over, or "start over
  as a common sailor", look like?
- How long should port actions take? Time passes in port (decided), but
  for now trading, hiring, and signing contracts are instant.

---

## 9. Creative License Log

Every deliberate deviation from history or physics goes here, with a reason.

| Deviation | Reason |
|---|---|
| Regional geography drawn at 0.5× real distance (`distance_scale` in each map file). Ships stay true scale | A crossing of the Port Royal approaches takes minutes rather than an hour. Open-ocean passages will need more compression, plus time acceleration. |
| The world begins on 1 June 1716, and the player joins it 18 hours later, after the 1692 earthquake and 1703 fire at Port Royal, with Nassau a pirate haven | Keeps Port Royal alive in our era and puts the player in the Republic of Pirates. |
| Coastlines are hand-traced approximations | First pass. To be refined from real coastline data. |
| Ports "produce" goods that really arrived by off-map shipping, such as Kingston's provisions and manufactures from New England and London | Until the map reaches those sources, someone has to supply them. |
| The Bahama Banks are impassable shoals, and the bank edges are approximate. Small sloops really could cross them | Keeps deep-water routes historical and simple. Shallow-draft sailing can come later. |
| Nassau salt comes from New Providence itself. Bahamian salt was really raked on the Exumas, Inagua, and the Turks | One port stands in for the islands around it. |
| Dates use the Gregorian calendar. England still used the Julian calendar, 11 days behind | Simpler, and Spain and France were already Gregorian. |
| A full hold slows a ship by up to 25% | Gives raiders sailing light an edge over laden merchantmen, so chases can be won. |
| A ship's crew and gunnery are averaged rather than rolled at sea; only battles by rounds use dice | Predictable, testable broadsides. The abstract style adds the luck. |
| Captured crews are set adrift in the boats and row for the nearest port at a steady meter a second, all but her mate, who's held as a prisoner | No prisoners to feed or ransom yet; the mate is worth recruiting, and the rest carry word of the taking. |
| Without depth data, any water within 5 km of land or a shoal counts as shallow enough to dive on | The map has no soundings yet. |
| Reefs strike at a flat chance near shoals | Stands in for weather until storms exist. |
| News spreads by whole ports: once known in one, every captain there can hear it, and every ship leaving carries it | Simple to follow, and close enough to how harbour talk worked. |
| Mutineers vanish with the ship they take | No pirate captains to sail her yet. |
| NPC captains burn the ships they take | Fleets of NPC prizes would need captains and orders of their own. The cargo and the port's need are what matter. |
| A town rises with one ship of the nation it favours in harbour, without a landing or a siege | Lets a single captain, or a raider, decide a port's fate; assaults can come later. |
| Relations between nations drift back to their 1716 footing | Keeps one bad month from locking the map into permanent war. |
| The Brethren count as a nation with ports, colours, and relations | Simplest way to put pirate havens and their politics on the same footing as the crowns'. |
| Officers each bring one fixed advantage, with no skill levels | Easy to read and to weigh. |
| Each nation is the same three groups, and Acts of Grace come round on a fixed calendar. The real King's Pardon was proclaimed once, in September 1717 | Uniform rules are easy to learn, and a known date gives a wanted captain something to plan for. |
| A pardon is granted the moment you dock under an Act, or pay a lawyer | No waiting on letters from London or Madrid. |
| A group the player serves hears of every sailing that concerns it the moment the ship sails, wherever she sails from | Intelligence that arrives too late to act on isn't worth having. |
| Merchants ask for an escort only when the player lies in the port they sail from | There's no one else to ask yet, and a request nobody can take is noise. |
| An arrested captain is fined and freed the same day | No jail time to sit through. |
| Ships sail straight on any heading, never tacking. Wind only scales speed, from 0.75× head to wind to 1.2× dead downwind | Beating to windward was realistic but made passages tedious. |
| Every shipyard sells every class at once, from schooner to frigate, takes your ship in part exchange, and the new one keeps her name | A new ship should be a purchase, not a wait for one to come up for sale. |
| Slave trade represented only as historical context (faction behavior, port descriptions), never as player cargo | Content choice. |

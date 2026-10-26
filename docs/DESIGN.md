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
  fire starboard, board, battle style, follow ship, strike colours, on
  keys 1 to 6) and a
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

- **Era: an age, not a year.** The world isn't set in any one year. It
  draws on the whole age of sail, from the caravels of the mid-1400s to
  the 1820s, when steam and the navies' last campaigns against piracy
  ended it: Portuguese and Spanish discovery, the buccaneers, the trading
  companies at their height, the pirate republic of Nassau, and the great
  powers fighting over colonial trade, all at once. Things from different
  decades stand side by side when that makes the better game: Port Royal
  stands, though it sank in 1692, while Nassau is a pirate republic, as
  it was from 1716 to 1718.
- **Accurate in detail, free in time.** Ships, ports, goods, names, and
  how things worked are as true to history as we can make them; what's
  mixed is *when*. Where gameplay asks for more than that, we take the
  license and write it down (§9). Nothing from after the age of sail:
  no steamships, no ironclads, no rifled guns.
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

The VOC (Dutch East India Company), the Dutch West India Company (WIC),
the English East India Company, the French Compagnie des Indes, and
smaller chartered ventures. Companies whose business was the slave trade
are left out (§9). The WIC is the first to come, in the West Indies
(§7, Milestone 8); the VOC comes with the East Indies.

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
| **Navigation** | Orders (move, follow, dock, engage). The crew steers each leg of a route planned around land. A ship aground first works off into open water, then goes on. |
| **Combat** | Built (§5.5): broadside arcs, reload time, separate damage to hull, rigging, and crew, boarding, and prizes, in three battle styles. Later: shot types (round, chain, grape), and holding the weather gage. |
| **Ports** | Docking, markets, shipyards, taverns (crew), and faction offices (contracts, commissions, fences). |
| **Economy** | Goods with per-port supply and demand. Prices respond to trade flows. Regional specialties include sugar, rum, tobacco, spice, and cloth. The slave trade is left out of the game; see §9. |
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
  - A chase that hasn't come to action costs 2 an hour (§5.7, "A stern
    chase is a long chase"), and a crew's morale sets how long its captain
    keeps a chase up.
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

#### The first ports

| Port | Produces | Needs | Notes |
|---|---|---|---|
| **Port Royal** (England) | soldiers, sailors | provisions, naval stores, munitions, rum, timber | The Royal Navy's Caribbean base. It eats stores and sends out soldiers. |
| **Kingston** (England) | sugar, rum, molasses, pimento, ginger, indigo, hides, logwood; and, shipped in, provisions, manufactures, naval stores, munitions, timber | soldiers, salt | Jamaica's commercial port. Sugar dominated the island; Jamaica was almost the only source of pimento; logwood was cut in Central America and re-exported through Jamaica. Provisions came from New England and Ireland because sugar land wasn't wasted on food. |
| **Nassau** (pirates) | salt, sailors | provisions, rum, munitions, naval stores | The Republic of Pirates, 1716 to 1718. No governor and no exchange: business is done in the taverns. |
| **Santiago de Cuba** (Spain) | hides, provisions (salt beef), timber, soldiers, sailors | manufactures, munitions, naval stores, salt | Spain's nearest port to Jamaica. Its own fleets come seldom, so it buys Jamaican manufactures, smuggled or not, and its guarda costas hunt the smugglers. |

Spanish silver, smuggled in from the Spanish Main, is left out for now:
silver is money, so it needs its own treatment as plunder rather than as
freight. The slave trade is left out of the game (§9).

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

Right-click a ship at sea to engage her, within 3 km: farther off, a
notice says how far she is, and you keep your course. Anyone can be
attacked; standing and reputation come with factions.

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
- **Leaks and pumps.** Every wooden ship takes water: a sound one only
  seeps, and the more of her timbers are gone, the faster it comes in.
  Her men pump as hard as they work her sails (the same share of her
  minimum crew, morale, and sailing master), and a whole company holds
  the water until she's down to a fifth of her timbers. Fewer hands lose
  ground to it sooner, and with no one at the pumps even a sound ship
  founders in about a day. Water gaining on the pumps eats her timbers,
  and the captain is told when it starts. Out of a fight she founders at
  nothing; in one, the enemy sank her. In port the harbour pumps her out.
  So a battered ship that has lost half her men makes for a yard, or
  goes down on the way.
- **Striking.** The strike control (6) strikes your colours in a fight,
  and you strike on your own when no one is left aboard to fight her.
  A struck ship furls her sails, and can't sail or fire until she
  hoists her colours again, once no one is engaging her.
- **The enemy.** An attacked captain sizes you up (guns, stoutness,
  damage, and hands): if she's at least as strong, or you're badly battered, she fights broadside-on. Otherwise she
  runs, downwind if that opens the distance, firing as her guns bear.
  Beaten (timbers, rigging, or crew), she strikes her colours.
- **Speed.** A full hold is up to 25% slower, so a raider sailing light
  catches a laden merchantman. One in ballast may get away.
- **Boarding.** Within 60 m, the board control (3) boards. A struck ship is carried without a
  fight. Otherwise crews fight it out, and defenders count for 1.25 men
  each. Morale and a first mate make each man count for more or less. The victor takes her purse and as much cargo as fits; her
  contracts are void. Other captains board a ship that strikes to them,
  and burn her.
- **Losing your ship.** Taken by boarders, you're put in the ship's
  boat with the men they spared. Foundered or wrecked, you take to it
  with your men. Struck with no one left aboard and no one coming to
  board her, you take to the boat with your strongbox and leave her a
  *derelict*: under no colours, struck for good, filling as she lies.
  Whenever the boat has no one to row it, whoever picks you up puts
  you ashore at the nearest port. Sunk in battle, all is lost.
- **Derelicts.** A struck ship with no one aboard can be engaged, to lay
  her alongside and board her; that's no attack on anyone. Your spare
  hands sail her as a prize, as with any other.
- **After a fight.** Your ship, if her fight ends with no course of her
  own, heaves to rather than sail on blind. Other captains' business
  gives them a course, and one that was running keeps running until it
  does (stopping her made raiders fall on the same merchant again and
  again).
- **Prizes.** Two spare hands (beyond the least that can sail your ship) sail her as a
  prize: a prize crew works her home under easy sail, so two sail her as
  well as a full crew. She
  follows you and docks where you dock. At a shipyard you can sell her,
  or take command of her and leave your old ship as the prize. With no
  hands to spare, she's plundered and let go.
- **Shipyards** repair timbers and rigging for a fee, in proportion to
  her price, and sell new ships (§5.1). The other captains' ships lying
  at a yard are made good at their owners' charge, half a hull a day.
- **Seamanship.** Every captain but the player keeps her ship fit and out
  of harm the same way, whatever her business: merchant, raider, or
  man-of-war. It takes the helm from her business whenever it must, and
  hands it back once she's fit and safe.
  - *Refitting.* A ship with timbers or rigging below 40%, or fewer men
    than she needs to work her, puts in at the nearest yard that will have
    her: her own nation's for a navy's, or any that welcomes her colours.
    The yard makes her good, half a hull a day; a navy mans her to the
    full at its charge, and anyone else hires the hands she needs from the
    tavern, as many as she can pay for. Her business may want more hands
    still, as a raider wants boarders, and hires them itself.
  - *Running.* A merchantman or raider runs from a warship that would hunt
    her, straight away or as near it as open water allows, until she's out
    of sight. A man-of-war stands. A raider whose lane a warship has found
    gives up her cruise.
  - *A stern chase is a long chase.* A hunter, warship or raider, gives
    up a ship she hasn't come to action with, and lets her be for a day,
    after as long as her crew will stand for: 4 hours for a steady crew
    (morale 45), in proportion to morale, between 1 hour and 8. A quarry
    that slips out of sight is remembered for an hour; seen again within
    it, it's the same chase. (Without that memory, a quarry that kept
    slipping over the horizon and back started a fresh chase each time,
    and a soak found a patrol chasing one raider for 26 days, 800 km off
    the map.)
  - *A fruitless chase wears the men down.* While a hunter is after a ship
    and not yet in action with her, her crew loses 2 morale an hour, so
    the longer a chase goes on, the sooner they'd give it up.

  Each rule is written once and every kind of ship follows it, so a new
  kind of ship needs only her business. The soaks found why: each rule
  was missing from one kind of ship or another, and each time it made a
  loop. A raider shot to pieces, or with 3 men after manning her prizes,
  struck to every merchantman she attacked and was let go to attack
  again, up to 3,200 times a soak. A patrol with her timbers shot away
  did the same 400 times, and a raider homeward bound past a patrol ran,
  turned for home into the patrol, and ran again, 2,000 times, until both
  were 3,500 km off the map.
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
  up, and at peace between. At the start England and Spain are at an
  uneasy peace (−20), the Brethren are hostile to England (−30), and at war
  with Spain (−60). Any ship's deeds under colours move relations
  between her colours and the nation she touched: attacking −2, taking
  −3, sinking −4, delivering to another nation's port +1, and lying in a
  port as it rises against its nation −25. Nations already at war take
  no further offence. Relations drift back to their opening footing by 0.5
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
  market, wins favour for the colours she flies: 4 for filling the whole
  gap below target of one good, in proportion, as a matter of course.
  Relief is worth more: up to four times as much for a good the port has
  gone short of for 30 days, in proportion to how long. Routine trade
  costs no one; relief costs the other nations half what it's worth
  beyond routine. Only the gap counts, so flooding a full market wins
  nothing. This is how pirates selling plunder win over a town, and how
  a nation that lets a port go hungry loses it.

  The steps are small beside the bands on purpose. Supply used to win
  25, half the way from a stranger's 25 to the 50 that opens a town to
  an enemy, and a quarter of the scale: one cargo could turn a port.
  Now even the greatest relief, a whole shortage filled after a month
  without, wins 16, and a routine cargo filling half a gap wins 2. A
  port is won over by a run of deliveries against settling's pull back
  to 25: holding a rival at 50 takes about a relief delivery a week.
- **Tiers.** How firmly a port is held is set in the map:

  | Tier | Ports | Restless below | Rising a day, no garrison |
  |---|---|---|---|
  | **Capital**, a nation's seat | Port Royal, Santiago de Cuba, Nassau | 10 | 5% |
  | **Colony**, a town like most | Kingston | 25 | 40% |
  | **Haven**, a lawless anchorage | Tortuga | 35 | 60% |

  A capital holds through all but utter neglect, and even restless rises
  only slowly. A haven goes to whoever looks after it. The first soaks
  had England take Nassau and then Santiago, Spain's only port, within
  45 days, by trade alone: tiers keep each nation its seat, and leave
  Tortuga as the port meant to change hands. Nassau is a capital for
  the game's sake, not history's: it's the Brethren's one safe harbour.
- **Convoys from home.** Every 20 days a convoy brings each of a crown's
  capitals a third of the stock it wants of everything it needs, as far
  as it wants. They still go short between convoys, so carrying for them
  still pays, but the crown no longer forgets its own seat. The Brethren
  have no crown to send them anything: Nassau lives on plunder. Their
  raiders are out for what Brethren ports are short of: they lie in wait
  three times as often on the lanes from ports that make it, and home from
  a cruise make for the nearest port that would have them and is short of
  something aboard, rather than just the nearest.
- **Restless.** Below its tier's mark for its owner, a port is restless:
  news in the HUD, and its garrison deserts at two men a day.
- **Going over.** A restless port rises for the nation it favours most,
  if more than its owner, when an armed ship of that nation (a warship,
  a raider, or the player) lies within 1.5 km of its anchorage and none
  of its owner's does. Trade wins a town over, but only guns raise it:
  a merchantman in harbour does nothing. The daily chance is its tier's
  with no garrison, less as its soldiers near their target, and none at
  full strength. The port takes its new flag with 60 favour for its new
  nation and 10 for its old one. The ship that was there is credited:
  −25 relations between the nations, and if she's the player, −15
  standing with the old nation (+5 with the Brethren).
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
  standing, between 40% and 95%. Only captains hear it: officers and
  crew hear nothing, and ships passing at sea don't speak each other
  (both planned, Milestone 7).
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
  crew, and damage at its start and end, every chase, every ship that goes
  down (how, and the crew and timbers she had left), the player's standing as it
  moves and why, her money, orders, controls, and alerts, nations coming
  to new terms, ports going over, and a snapshot of the world once a game
  day (relations, ships by role, every port's prices, memory). It's only
  in builds with the `chronicle` feature (`dev` has it). The last 20 runs
  are kept, up to 200 MB.
- **Notes.** F8 pauses and opens a box to type a note in, kept with a
  snapshot of that moment.
- **Knobs and targets.** The numbers worth tuning live in
  `assets/data/tuning.ron` and what a good soak looks like in
  `assets/data/balance.ron`, each with hints on what it does, how it
  changes play, and its history. The logbook's Tuning page edits both
  and shows each target against the newest soak, and each soak's summary
  lists the targets it missed. See `docs/BALANCE.md`.
- **Alarms** are raised once each when something looks broken: a ship with
  a route that hasn't moved in a day, a position that isn't a number, a
  port with negative stock or a month without a good it needs, a war two
  months long, the player's standing pinned at ±100.
- **Crash reports**, in every build: a panic writes what panicked, the
  backtrace, and the run's last 300 records, with a copy of the quicksave,
  to the crashes folder. Warnings and errors the game logs are kept too.
- **Soaks.** `cargo soak` runs a new game's world headless with no one at
  the helm, as fast as it goes (100 days in about a minute), chronicled.
  Each casts the world's dice from a fresh seed, so no two run alike; the
  log and the logbook keep the seed, and `cargo soak --seed N` plays a
  run again. Soaks can run side by side: a log is named for the second
  its run began, and one begun in the same second as another takes the
  next name (`…-soak-02`), never writing over a log already there.
- **Supply and demand.** Every run's summary and page list each good
  with the ports that make it and how much a day, the ports that use it,
  what convoys land at most, and whether the world is short or over,
  shortest first. Ships' own victualling isn't counted. It's how a map's
  economy is balanced: see Milestone 6.
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
   June, with months but no years (§3).
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
5. **Captains read their crews.** A fruitless chase already wears morale
   down, and morale already sets how long a chase is kept up (§5.7).
   The rest of it:
   - *Long cruises tire men.* After two weeks or so from port, a crew's
     resting level at sea slowly falls, so a cruise has a natural length
     and a run ashore is worth paying for.
   - *A sullen crew wants port.* An NPC captain whose crew is sullen ends
     her cruise or patrol and makes for port, as she does when she's
     battered (§5.7's seamanship), and gives them a run ashore there.
   - *NPC mutiny.* NPC ships get officers and the same mutiny rules as
     the player's (§5.9): a captain who drives a mutinous crew too long
     may lose her ship, and a pirate crew may turn a navy prize-master
     out. One rule for every ship.
   - *Temperament.* A captain's nature, as data on her: dogged ones keep
     a chase up longer, cautious ones give up and run sooner, rash ones
     board when they shouldn't. It scales the same rules rather than
     adding new ones.

### Milestone 5 — "Bigger ships"
1. ✅ Ship classes as data, NPC fleets of mixed classes, stoutness in
   battle, buying ships at the shipyard, and models rigged by class (§5.1).
2. Refits and upgrades: a few per hull, each one fixed advantage (copper
   sheathing, more guns pierced, a bigger hold, more berths).
3. **Shot types, and where shot lands.** Each hit already lands in one
   of three places: the hull (timbers), the rigging, or the men. Shot
   types choose the aim, rather than modelling where each ball strikes:
   - *Round shot* for the hull, as now.
   - *Chain shot* for the rigging, to slow her and stop her running;
     shorter range.
   - *Grape* for the men, before boarding; short range only.
   - *Between wind and water.* A dice roll sends some hull hits low,
     each opening a leak of its own on top of her damage (§5.5's leaks
     and pumps), so a lucky broadside can hole a sound ship. Her
     carpenter plugs them, one at a time.
   Loaded shot is chosen on the action bar; changing it takes a reload.
4. Better ship models and art.

### Milestone 6 — "Lines of supply"
The world must make what it uses. The logbook's supply-and-demand table
(§5.13) showed it doesn't: in October 2026 the ports used 3.1 tons of
munitions a day against 0.5 made and 0.8 landed by convoys, and naval
stores 3.8 against 1.5 and 1.1, so Nassau and Port Royal went without
for months, and raiders had next to nothing to take. Meanwhile Kingston's
sugar, molasses, logwood, indigo, ginger, and pimento piled up with no
port to use them. Every change here is checked against that table.
1. **Supply fleets from home, as ships.** Convoys (§5.10) sail in from
   the map's edge to a crown's capitals and are seen doing it: "the
   Jamaica fleet is in" is news in the tavern, and the player can see
   the market about to move. They carry home what the colonies make, so
   exports have somewhere to go. Their escort is strong enough that
   attacking them is suicide; for now they can't be fought at all.
2. **The smugglers' run.** Traders out of Carolina and New England sail
   in from the edge to Nassau with powder, shot, and naval stores, and
   buy plunder: historically, the colonies kept the Republic of Pirates
   supplied. It gives the Brethren a supply that isn't piracy, and a
   lever for diplomacy later: England pressing its colonies to stop the
   trade starves Nassau.
3. **More ports that make things**, and more made at the ports there
   are, balanced against the table, so that trade routes are worth
   setting up, protecting, and raiding.
4. **Darkness.** Every ship now sees every other within 8 km, day and
   night. Sight should fall at night: perhaps 8 km by day and 2 to 3 km
   on a dark night, more under a moon if the moon is worth having. A ship
   can be ordered darkened, her lanterns out, to be seen less far still,
   at some cost (slower, or a risk of collision). Slipping through a
   blockade at night then needs no special rule: she times her run and
   steers between the guard ships. It helps raiders and smugglers
   everywhere, not only at blockades.
5. **High land blocks sight.** Sight is a plain 8 km circle today, so
   ships see each other through islands and headlands. Found in a
   playtest (October 2026): the Ranger, in Kingston harbour, sighted
   the player running along the far side of the Palisadoes and came out
   after her. That case is fair, since the spit is a few meters of sand
   and a lookout aloft sees topsails over it. A ridge or a
   mountainous island is not. So land is marked high or low in the map
   data (high by default; low for spits, cays, and flat shores), and two
   ships don't see each other when the line between them crosses high
   land. One sight rule, used everywhere sight is used: hunting and
   running, witnesses to deeds, sightings, escorts, wrecks. It goes
   with darkness: the two together decide who sees whom, and a
   headland becomes somewhere to hide behind.
6. **Blockades.** Warships of a nation at war with a port's owner, lying
   off its anchorage, blockade it: ordinary trade can't reach it, so it
   starves. Supply that gets through is relief (§5.10), so the runner
   wins real favour there, and the blockading nation none. The navies'
   guard duty (§5.7) is the start of it.

### Milestone 7 — "Running the blockade"
Blockade running kept Nassau and Charleston alive, and makes the stories
the game is for. It builds on darkness and blockades (Milestone 6) and
on word of mouth (§5.12).
1. **Coves and longboats.** A few landing places along the coast near
   each port, in the map data: coves, beaches, creeks. A ship anchors off
   one and lands her cargo by boat, a few tons an hour, exposed while she
   works; darkness makes it safer. What's landed reaches that port's
   market, at a smuggler's discount, with some risk of its being found
   and seized.
2. **Arranged drops.** Word is sent ashore, through a friendly tavern or
   a fishing boat, and a rendezvous is fixed: the council will meet her
   at a cove on a set night. The arrangement is news (§5.12), and news
   travels: an officer with hidden loyalty (§5.9), or someone in a
   friendly port's tavern who wants favour with its nation, can tell the
   blockaders, and their navy sends a ship to the cove. She may come in
   to find a frigate waiting. The talk tells it vaguely at first
   ("something's being run ashore on the north coast"), the way taverns
   get a cargo wrong, and truer the more it's repeated; the navy sends
   its nearest patrol to stand guard, as patrols already do over a wreck.
3. **Officers hear the talk.** Today only captains hear a tavern, and
   officers act on rules alone (a turncoat sells his captain out when she
   docks where she's crossed). Officers ashore in a port should hear what
   it talks of too, and their hidden loyalty decides what they do with
   it: a turncoat aboard sells the plot to the blockaders, an ambitious
   lieutenant of the blockading navy carries it to his superiors for
   favour, a loyal one keeps it. Crew stay a number; the officers are
   the ears.
4. **Speaking ships.** Ships learn nothing at sea today but what they
   see. Two ships of one flag, or friendly flags, passing close "speak"
   each other, hailing or closing to talk, and trade what each has
   heard. Word then travels faster than port to port, so a plot has a
   reason to keep quiet and a blockade runner a reason to keep clear of
   anyone. Flag signals stay crude and navy-only (Popham's code is 1800;
   no Morse, which is post-sail).


### Milestone 8 — "New flags"
France and the Dutch Republic join England, Spain, and the Brethren, with
the ports to make them matter. Each nation is data (factions, groups,
fleets) and each port is data (map), so most of the work is ports, art,
and balance against the supply table.
1. **France, on Saint-Domingue.** The western third of Hispaniola,
   beside Tortuga and already on the map: Petit-Goâve and Léogâne, and
   Cap-Français on the north coast. Indigo and sugar; wants provisions,
   manufactures, and naval stores. Groups: the Marine, the Governor of
   Saint-Domingue, the merchants. A fourth crown with interests that cross
   everyone's.
2. **More of Spain.** Havana as Spain's real seat in the region and the
   treasure fleet's gathering point, a better capital than Santiago;
   Santo Domingo on Hispaniola; later San Juan and St. Augustine. A
   wrecked treasure fleet off Florida, after the 1715 fleet, whose
   salvage camp Henry Jennings raided in 1716: it fits the wreck and
   diving rules (§5.12) as they are.
3. **The Dutch Republic, trading first.** Curaçao (Willemstad), the great
   smuggling market of the Spanish Main, and St. Eustatius ("Statia"), a
   free port where anyone could buy powder and shot. Neutral in most
   wars; their ports deal with nearly anyone. This needs the map
   extended east and south, toward the Lesser Antilles and the Main.
4. **The Dutch West India Company (WIC)**, the first trading company
   faction (§4.2): its own factors, ships, and contracts, loyal to
   money, hiring anyone useful. In the West Indies it's the WIC; the VOC,
   the Dutch *East* India Company, belongs to the East Indies region
   (Later), and comes with it.
5. **A neutral port.** Danish St. Thomas (Charlotte Amalie) traded with
   anyone, pirates included: somewhere for enemies to meet besides
   favour.
6. **More of England**, as the map grows: Barbados, Antigua (English
   Harbour, the navy's other base), Bermuda, and Charleston, home port
   of the smugglers' run (Milestone 6).

Portugal never held Caribbean colonies: its Atlantic empire was Brazil
and the West African coast. It belongs to a later region, with Brazil's
gold fleets as a prize, and its caravels and navigators belong to the
age the game draws on.

### Milestone 9 — "Governor"
Trade as a way up, while she still sails. It needs fleets (Later) first,
since a route is protected by ships under her command.
1. **Her own trade routes.** A standing contract she sets up between two
   ports: her own merchantmen sail it, she or her escorts guard it, and
   rivals and raiders prey on it. The trading companies' path (§4.2):
   contract hauler, company captain, factor.
2. **Governorship.** With enough standing with a nation's governor and
   enough favour in one of its ports, she's offered the port: she sets
   its garrison, its taxes, and the routes it commissions, and answers
   for it going hungry or restless. The Brethren's equivalent is a seat
   on the Nassau council. A governor's routes still need defending, so
   it draws her back to sea, not into a management screen.

### About the history
A short note in the game, reached from the main menu (with the credits),
saying plainly that the trade and wealth of this period rested on the
Atlantic slave trade and the enslavement of millions of Africans, that
the game leaves it out deliberately rather than make it a mechanic, and
pointing to places to learn about it properly: for example the
International Slavery Museum in Liverpool, the Smithsonian's National
Museum of African American History and Culture, and the Trans-Atlantic
Slave Trade Database (slavevoyages.org). Links chosen and checked when
it's written. Small, and can go in whenever the menu has room for it.

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
- Should the supply fleets from home (Milestone 6) ever be fair game?
  Taking a treasure fleet is the great pirate prize, and could be a
  late-game goal for a captain with a fleet of her own.
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
| The world has no year. It draws on the age of sail from the mid-1400s to the 1820s, and things from different decades stand side by side: Port Royal stands, though it sank in 1692, beside Nassau's pirate republic of 1716 to 1718 | Lets the game have the best of the whole age: the buccaneers, the pirate republic, the trading companies, and every navy at its height. Details stay accurate; only *when* is mixed. |
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
| Relations between nations drift back to their opening footing | Keeps one bad month from locking the map into permanent war. |
| The Brethren count as a nation with ports, colours, and relations | Simplest way to put pirate havens and their politics on the same footing as the crowns'. |
| Officers each bring one fixed advantage, with no skill levels | Easy to read and to weigh. |
| Each nation is the same three groups, and Acts of Grace come round on a fixed calendar. The real King's Pardon was proclaimed once, in September 1717 | Uniform rules are easy to learn, and a known date gives a wanted captain something to plan for. |
| A pardon is granted the moment you dock under an Act, or pay a lawyer | No waiting on letters from London or Madrid. |
| A group the player serves hears of every sailing that concerns it the moment the ship sails, wherever she sails from | Intelligence that arrives too late to act on isn't worth having. |
| Merchants ask for an escort only when the player lies in the port they sail from | There's no one else to ask yet, and a request nobody can take is noise. |
| An arrested captain is fined and freed the same day | No jail time to sit through. |
| Ships sail straight on any heading, never tacking. Wind only scales speed, from 0.75× head to wind to 1.2× dead downwind | Beating to windward was realistic but made passages tedious. |
| Every shipyard sells every class at once, from schooner to frigate, takes your ship in part exchange, and the new one keeps her name | A new ship should be a purchase, not a wait for one to come up for sale. |
| The slave trade is left out: no enslaved people as cargo, no slaving ships, contracts, markets, or companies whose business it was (so no Royal African Company, and no South Sea Company, whose foothold in the Spanish colonies was the right to sell slaves there) | The Atlantic economy of the period rested on it, and the game won't make it a mechanic or a source of profit. It's acknowledged plainly, not hidden: see "About the history" in §7. |
| Tortuga is a pirate haven, as it was for the buccaneers of the 1630s to 1660s, beside the French Saint-Domingue that later absorbed it | Both were true, decades apart. Decide when France arrives (§7, Milestone 8): keep it the Brethren's, or hand it to France and find them another haven. |

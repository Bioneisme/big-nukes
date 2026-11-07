# API notes

Findings about Horizon's native mod API that the code relies on. Horizon Nightly 2026-11-05 (66f3981), API 411.

## The assets half

The native half knows the assets half is installed when the loot table `big_nukes:marker` exists
(`world_items!(game).has_loot_table`): the mod has no block of its own yet, whose loot table would do.

## Bulk edits

- `native_edit_blocks` (API 126) only edits the level being ticked, so the waves work in `native_level_tick_post`.
- A run's `surface` height is one above the column's highest block on that heightmap, read once a column before the
  edit changes anything. So the scorch is a second edit, after the plants are gone.
- Unloaded chunks are skipped without an error: the mod tracks loaded chunks itself (`native_chunk_load`/`unload`,
  API 57) and defers the rest.
- In 26.3, `#minecraft:dirt` is only dirt, coarse dirt and rooted dirt; grass, podzol and mycelium are
  `#minecraft:grass_blocks`, and `#minecraft:substrate_overworld` holds all of them with mud and moss.
- `#minecraft:replaceable` holds water and lava, so plants are named one by one.
- An edit reads its whole palette before its runs, and one entry that doesn't read refuses it all; so an edit with no
  runs tells whether a block is in the game. The scorch asks so once a world for `big_nukes:ash` (the assets half).
- The game builds with `-D warnings`: an unused constant or a never-built enum variant in the rules fails the build.

## The blast's effects

- `native_gui_render_post` (API 410) has no camera: `native_camera()` answers None there, so the flash reads the player's
  eyes (`native_player`). It does draw under F1.
- `native_run_command` answers false when a command affects nothing (a damage band with no one in it, smoke no player is
  near enough to see), so false is not logged as an error.
- A mod's `data/<ns>/damage_type/*.json` in the `.hmod` was not found by `/damage` on Nightly 66f3981 ("Can't find
  element 'big_nukes:blast' of type 'minecraft:damage_type'"), so the blast does `minecraft:explosion` damage. The type
  is loaded; only the command's check refuses it (a Horizon bug, written up in `docs/horizon-issue-damage-types.md`).
  Radiation adds `big_nukes:radiation` to `game.ui.registries.by_registry` itself (`known_damage_type`, not an API).
- Sounds are the mod's own (`tools/make_sounds.py`). A local sound reaches `max(volume, 1) × attenuation_distance`
  blocks, so each is played 6 blocks from the player towards the blast, with the delay of the front.

## The bombs

- `native_right_click_block` runs once per hand of a click and has no hand of its own. A canceled main hand ends the
  click, and the glue lets go of a second turn of the same player on the same bomb in the same tick. The main hand's
  stack is `native_equipment(EquipmentSlot::MainHand)`; the player is `native_acting_player`.
- `native_run_command` answers false inside a player's hook, so sounds and particles go through
  `native_level_play_sound` and `native_level_send_particles`. A worn flint and steel is written back through the
  inventory tick (see "The held stack").
- A mod block's items spill when it is broken only if its block has a `ModContainer`: each bomb has one hidden slot for
  its detonator, with no faces, inputs or outputs, and the keypad's menu type (`big_nukes:keypad`, 1 slot, 4 data
  slots). A block the mod removes during its own tick drops what is left in `items`, so a bomb clears its slot first.
- The block entity's tick knows its level from `native_dimension`; `native_set_block` there takes the bomb away.

## The remote

- The link is an item component: `native_component_types` (API 65, an associated function with no `self`) registers
  `big_nukes:link`, and `native_set_component`/`native_component` write and read it as NBT (a compound of
  `dimension`, a string, and `pos`, an int array). The stack written goes back to the main hand through the inventory
  tick (see "The held stack"). The component's types are process-wide, so the window's tooltip reads it too.
- `native_use_item` (API 13) runs for a right click in the air and for one on a block whose use let it through; it
  has the hand's stack but not the hand, and the acting player is `native_acting_player`. A click on a bomb with the
  remote doesn't reach it, as the bomb's right click is canceled (seen in the game: remote_test links by that click).
  `native_use_item_on` runs first for a click on a block (`game.rs`, the item phase), and a claim there skips
  `native_use_item`; the mod claims every remote click on a block, so the remote fires only aimed at the sky.
- A bomb set off from afar is taken when its wave works the chunk of its center, as the crater stops below the
  Nether's roof and a bomb above it would otherwise stay to be linked again (found in the final review).
- `native_block_changed` is `GameLevel`'s, not `Game`'s, and runs for every `Level.setBlock`. A bulk edit
  (`native_edit_blocks`) writes as `fill ... strict` does and reports nothing, so a blast's crater is checked for the
  recorded bombs after each chunk. The level may set a block while the mod's state is locked (a command run during a
  level's tick), so a changed bomb is queued and forgotten the next time the state is opened.
- `native_set_block` and `native_with_mod_block_entity` only reach the level in place: a remote takes its bomb away
  at once only in a loaded chunk of the player's own level; elsewhere the bomb goes with its wave's crater.
- Which levels tick (`levels_to_tick` in `game/levels.rs`): those with a player, a bed's owner, forced chunks
  (`/forceload` or a mod's tickets), a pending portal arrival or a game test; the overworld when nothing else does.
  A level nobody is in closes after 300 idle ticks. `execute in <dim> run forceload query` opens a closed level
  (`enter_command_level`), which then ticks only if it has forced chunks. So the remote opens a bomb's other
  dimension that way, for an operator's forced chunks there, and otherwise the wave waits: its front is reckoned from
  the world's clock, and every chunk it reached unloaded is written when it loads. `native_run_command` answers
  false inside a player's hook, so the command waits for the end of the server's tick.

## The sky

- The renderer's hooks (`native_world_drawn`, `native_render_needs`, API 58) are free functions in
  `horizon-client-render`, which can't see `crate::game`. The mod's `[[module]]` `big_nukes_sky` goes into
  `crates/horizon-client-render/src/gfx/native_render.rs` with `visibility = "pub"`, and the window's glue fills its `SKY`
  from `camera_shake`, once a frame with the part of the tick.
- The pass runs after the world's post pass and before the HUD, over the swapchain colour (`LoadOp::Load`), with
  premultiplied alpha. The depth is Depth32Float, reversed (sky = 0), sampled with `textureLoad`; `depth_size` can differ
  from `size`.
- The WGSL is a string in `sky.rs` (a spliced module can't `include_str!` a sibling), validated by naga 30.0.1 in the
  tests. A failed pipeline is logged and the clouds left out.
- Cost, offscreen, R=250 at 400 blocks: the cloud ahead adds about 0.9 ms to the median frame (32 steps); from inside the
  cloud, at 16 steps, the frame is 1.8 ms (GPU 1.7 ms). Some benches after a teleport showed 33 ms frames with the CPU at
  1.9 ms and the GPU at 5.7 ms: waiting, not drawing, seen as chunks load; worth a look in a real window.

## Crafting on the assembly table

Read in the source of 66f3981, not yet seen in the game:
- `native_prepare_item_craft` (API 290) is called only from `game/lan_crafting.rs`, for LAN guests: the host's own
  grid (`menu.rs`) never calls it, so the host crafts the bombs anywhere.
- The menu a block opens is chosen by its name, not its template: `Game::open_menu` sets up a 3×3 grid only for
  `minecraft:crafting_table`, and `lan_table_valid` closes a guest's table at any other block. A block on the
  `minecraft:crafting_table` template, `big_nukes:assembly_table`, may open no grid at all.
- So the bombs and their parts have no crafting table recipe: the assembly table is a machine of the mod's own, with
  a block entity, a 10-slot menu (`horizon/menu/assembly_table.json`) and an Assemble button. It keeps the
  `minecraft:crafting_table` template: that class's use opens a menu (`PlayerEffect::OpenMenu`), and `open_menu`, not
  matching the name, falls through to `open_mod_menu`. A block entity works on any template since API 129.
- A mod screen's button: the window's `native_screen_click` (API 20) sets a menu button id for a press over no slot,
  and the server hears it in `native_menu_button`, with the block entity out of the world during the call.

## Wave measurements

`tools/wave_bench.py`, 2026-11-06, `performance` build, offscreen, seed 12345 at spawn, render and simulation
distance 12, the budget per level tick 6 ms (`BUDGET` in glue.rs). "Done" is when the mod logs that the loaded part of
every wave is done; the chunks beyond the render distance wait for their load.

| Radius | Done | Median frame, base → wave | Worst frame | Server tick, mean / P99 |
|---|---|---|---|---|
| 50 | 1.2 s | 0.43 → 0.51 ms | 48 ms | 6.0 / 16.9 ms |
| 250 | 7.8 s | 0.51 → 0.62 ms | 50 ms | 16.6 / 34.1 ms |
| 1000 | 23.0 s | 0.50 → 0.54 ms | 50 ms | 4.3 / 6.3 ms |

With a 10 ms budget, radius 250 was done in 6.7 s with the median frame 0.48 → 0.63 ms (a 24% drop in FPS, over the
plan's 20%), so the budget went down to 6 ms. The mean FPS and the slowest 1% of frames vary a lot between runs at
these offscreen frame rates (about 2000 FPS) and are left out. Every run had one frame of about 50 ms around the
blast. At radius 1000 the loaded area is all crater, whose runs are unfiltered and cheap, so its ticks are lighter
than at 250.

With the effects on (plan 2), radius 250: done in 7.8 s, the median frame 0.52 → 0.86 ms, the worst frame 45 ms, the
server tick 16.2 ms on average and 32.0 ms at P99. The smoke and the flash cost about a third of a millisecond a frame,
which shows as a large share only at offscreen frame rates.

## The radiation's items

Read in the source of 66f3981, not yet seen in the game:
- An item's `components` is a 26.3 patch over its template's: a patched component replaces the template's whole one,
  and `"!minecraft:<type>": {}` removes one. `minecraft:max_stack_size` and `minecraft:rarity` are refused there: they
  are the item's own fields. The rules build each item's patch (`components` in runtime.rs), so the tests read it.
- The suit copies the iron armour and patches `equippable` (the whole map: `slot`, `asset_id`, `equip_sound`),
  `max_damage`, `attribute_modifiers` (armour 1, under vanilla's `minecraft:armor.<piece>` id for its slot) and removes
  `repairable`. Its look is `assets/big_nukes/equipment/hazmat.json`: every namespace's `equipment/` is read
  (`armor_stands.rs`), and the layer `big_nukes:hazmat` of kind `humanoid` is the sprite
  `big_nukes:entity/equipment/humanoid/hazmat`, 64x32 in vanilla's armour layout (`humanoid_leggings` the same).
- An `on_consume_effects` entry whose type is outside `minecraft:` is kept whole and handed to
  `native_consume_effect(kind, data)` (API 74), `data` being the entry itself with its `type`; the drinker is
  `native_acting_player`. The anti-rad is a paper with `consumable` and `use_remainder` (a glass bottle).
- A mod's mob effect is `data/<ns>/horizon/mob_effect/<name>.json` (`category`, `color` only); its HUD icon is the
  sprite `<ns>:mob_effect/<name>` and its name `effect.<ns>.<name>`.

## The held stack

- In a player's hook the host has no entity handle: `native_acting_player().entity_id` is 0 for it
  (`native_acting_entity` counts only LAN guests), so `native_living_set_equipment` can't reach the host's hand.
  Found in game: a remote never linked. The mod queues the change and makes it in `native_inventory_tick`
  (API 39) for the player's selected slot, where `native_carried_stack` changes the stack in place and the
  client gets it. The flint and steel's wear goes the same way.

## Radiation

Read in the source of 66f3981, not yet seen in the game:
- A player's dose builds in `native_player_tick` (API 31), once for each living player each server tick, with that
  player swapped in: `native_acting_player` gives the place, dimension and game mode, `native_equipment` the armour.
  `native_run_command` answers false there, so the sickness' commands wait for the end of the server's tick.
- `native_player_died` (API 40) runs inside the `/damage` that kills, which may be run while the mod's state is
  locked (a blast's damage in a level's tick), so a death is queued and the dose goes the next time the state opens.
- A hazmat piece's wear is queued as the flint and steel's is and made in `native_inventory_tick` for the armour slots
  (100 feet to 103 head, `selected` false there). Its durability is the item's `max_damage` from the world's items,
  600 when that is 0.
- `/damage` that does no damage fails (vanilla throws when `hurtServer` answers false), so a failed
  `big_nukes:radiation` did nothing and the same damage is tried as `minecraft:outside_border`: that covers a game
  whose command registry doesn't know the mod's damage type (see "The blast's effects"), with the world border's death
  message. Where the target couldn't be hurt at all, the second fails too.
- `native_mobs` (API 37) only answers inside its level's tick: the mobs are hurt in `native_level_tick_post`, once a
  second, at most 64 a dimension per pass, the rest on the next passes. A mob's `/damage` names it by its UUID
  (`native_entity` and `native_entity_uuid`, API 373) in its own dimension (`execute in`).

## Radiation in the window

Read in the source of 66f3981, not yet seen in the game:
- The window knows nothing of the zones or the dose: the server sends `big_nukes:rad` (the dimension's 32 nearest
  zones, the mod's clock and the dose, about 4 KB at most) once a second to each player with something to tell, and
  once more when there is nothing, so the window lets go. Between messages the window counts on with
  `native_clock().game_time`, which stands while single player is paused.
- `native_tick` runs in every `Game`, the single player server's too: the counter acts only where `native_window()`.
  `native_equipment(MainHand|OffHand)` reads the window's own player there.
- `native_gui_layer_post`'s ids carry the namespace (`minecraft:hotbar`), though its doc lists `hotbar`. A layer F1
  hides fires nothing, so the reading goes with the HUD.
- `native_render_fog` (API 117) is `&self` and comes every frame: the zone's fog eases by the time between frames, kept
  in a static. A change with a colour outside 0..=1, a distance that isn't finite or an end not past its start is
  dropped whole. With Horizon's Fog option off the distances are 1e9 and 2e9, and the mod leaves them.
- `native_world_lifecycle` is the server's: a LAN guest or a player on a server never hears it. The window's state goes
  in `native_client_level_unload` (API 135), told on a dimension change and on leaving a world.

## The silo and the bomber

Read in the source of 66f3981, not yet seen in the game:
- The flying missile, bomber and bomb are `item_display`s made with `native_summon` and moved every tick with
  `native_move_entity` (API 59) at the end of their level's tick, the only place the entity helpers reach. A
  display glides between places over its `teleport_duration` (2), and `view_range` 12 lets it be seen 768 blocks
  off. Summoning refuses a chunk that isn't loaded, so a missing display is made again on a later tick.
- Horizon draws an item display as its orientation, then its `transformation`, then half a turn about y, then the
  item (`text_displays.rs`, as vanilla's `ItemDisplayRenderer`): the models are written nose at -z, so the nose
  comes out along the display's yaw and pitch, and the summon's `transformation` scales them (a model can't pass
  3 blocks) and moves the missile forward so its tail is the display's place.
- The displays are saved with their chunks: each carries `big_nukes_flight`, its flight's and its session's tag,
  and the mod kills the tagged ones that belong to no flight of this session.
- A mod menu has 1 to 64 slots (`content.rs`): the console's menu has one hidden slot, as a keypad's.
- `native_screen_key_pressed_pre` and `native_screen_char_typed_pre` (API 104) tell a block's mod menu apart by
  `screen.menu` (`big_nukes:console`). A cancelled key still sends its characters, so the digits come through the
  character hook while every key but Escape is cancelled (no `e` closing it, no 1-9 swapping hotbar items).
- `native_net_send_to_server` needs the game mutably, so the screen's hooks queue a launch and `native_frame` sends
  it; `native_net_from_client` (API 40) runs with no level in place, so a launch is worked at the end of the tick
  of the player's level.
- A block menu's screen doesn't know its block's place: the server sends it to the window when the console is
  right-clicked (`big_nukes:console`).
- `native_camera_pose` (API 402) moves the picture only; the window reckons the missile from the flight it was told
  and its own `native_clock().game_time`, two ticks back for the display's glide.
- The target's chunk is kept with `execute in <dim> run forceload add`, run from a level's tick where commands run,
  and the world keeps the list (`Saved.forced`) to remove it once the blast's wave has worked its center's chunk.

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
  element 'big_nukes:blast' of type 'minecraft:damage_type'"), so the blast does `minecraft:explosion` damage. Plan 5's
  radiation damage has to find a way around this.
- Sounds are the mod's own (`tools/make_sounds.py`). A local sound reaches `max(volume, 1) × attenuation_distance`
  blocks, so each is played 6 blocks from the player towards the blast, with the delay of the front.

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

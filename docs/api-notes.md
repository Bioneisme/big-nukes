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
- The game builds with `-D warnings`: an unused constant or a never-built enum variant in the rules fails the build.

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

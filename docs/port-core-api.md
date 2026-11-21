# The core's surface for the stage-2 modules

`silo.rs`, `bunker.rs`, `decon.rs` and `events.rs` take what they share with the core from `crate::core`
(`use crate::core::{self, Pl, lock, ...}`) and the rules from `crate::rules`. `lib.rs` and the core already call
every function in those four files with the signature it has now: change the bodies, not the signatures. They
may call one another (`events` → `bunker::powered_now`, `bunker::shelter_of`, `silo::notify`; `bunker` →
`decon::quick_move`).

Old glue name → new, all in `crate::core`:

| Old (`big_nukes_glue::`) | New | Notes |
|---|---|---|
| `lock(&M)` | `lock<T>(m: &'static Mutex<T>) -> MutexGuard<'static, T>` | poison-tolerant |
| `state(game)` | `state(host: &dyn ServerHost) -> Option<MutexGuard<'static, Option<State>>>` | `guard.as_mut().expect("opened")`; fields `saved: rules::Saved`, `dirty: bool` |
| `save(st)` | `save(st: &mut State) -> bool` | |
| `clock(game)` | `clock(host: &dyn ServerHost) -> Option<(u64, u32)>` | `(Saved.tick, maxpower)`; opens the state |
| `middle(pos)` | `middle(pos: [i32; 3]) -> [f64; 3]` | |
| `loaded(dim, pos)` | `loaded(dimension: &str, pos: [i32; 3]) -> bool` | chunk heard loaded |
| `start_bomb(game, ..)` | `start_bomb(host: &mut dyn ServerHost, dimension: &str, center: [i32; 3], kind: rules::BombKind, radius: u32) -> Result<u64, rules::Refused>` | takes the state's lock: call it with yours let go |
| `start_blast(game, ..)` | `start_blast(host: &mut dyn ServerHost, dimension: &str, center: [i32; 3], radius: u32) -> Result<u64, rules::Refused>` | same |
| `tell_near(game, dim, pos, &line)` | `tell_near(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3], line: &Text)` | within 64 blocks; `tell_within(.., within: f64)` too |
| `refused(why)` | `refused(why: rules::Refused) -> Text` | |
| `stacks_with(a, b)` | `stacks_with(have: &ItemStack, stack: &ItemStack) -> bool` | same item and components |
| `burn_time(game, s)` | `burn_time(host: &dyn Host, stack: &ItemStack) -> i32` | 0 for no fuel |
| `ash_here(game, dim)` | `ash_here(host: &dyn ServerHost, dimension: &str) -> bool` | opens the state |
| `native(edit)` | `native(host: &dyn ServerHost, edit: rules::ChunkEdit) -> Result<BlockEdit, String>` | then `host.edit_blocks(dim, &edit)` from `level_tick` only |
| `assets_here(game)` | — | gone: the content ships in the package (drop the checks) |

Other helpers in `core` (they replace `game.native_*`):

- Players: `Pl { player: PlayerRef, uuid, name, dimension: String, position, game_mode, health }` with `exempt()`
  (creative/spectator) and `creative()`; `pl(host, PlayerRef)`, `by_uuid(host, u128)`, `acting(host)` (the old
  `native_acting_player`), `players(host)` (`native_server_players`), `held(host, &Pl)` (main hand),
  `sneaking(host, &Pl)`.
- Text: `tr(key, Vec<Text>)`, `lit(x)`, `colored(text, "gray")`; `tell(host, uuid, Text)`,
  `tell_everyone(host, &Text)`; `send(host, uuid, channel, Vec<u8>) -> Result<(), String>` (a mod message);
  `run(host, command) -> bool`.
- Window: `lang(host, key)`, `lang_has`, `lang_tr(host, key, &[&str])`; `argb(rgb)` (draw calls take ARGB).
- World: `block_at(host, level, pos) -> String` (`native_block`; air when unloaded), `mod_block(BlockKey)`,
  `item_key(host, id)`, `stack_of(host, id)`, `stack_of_n(host, id, n)`, `set_air(host, level, pos)`,
  `play(host, dimension, event, at, SoundSource, volume, pitch)`, `particles(host, level, &ParticleOptions, at,
  spread, speed, count)` (32 blocks; keep `particle … force` commands for far ones), `distance(a, b)`.
- Chunk tickets: `force_chunk(host, dimension, chunk, hold) -> bool` (the old
  `native_force_chunk_in(rules::TICKET_OWNER, ..)`; false where the level isn't open: open it with
  `run(host, "execute in <dim> run forceload query")` and try again next tick). `world::validate_tickets` keeps a
  saved ticket while `Saved::held()` lists its chunk.
- `crate::keys()`: `air`, `ash`, `geiger`, `remote`, `flint_and_steel`, `bucket`, `lava_bucket`, `hazmat`,
  `carried`, `iodine` (effect), `smoke`, `flame` (particles), `tickets`. Add a key in `lib.rs::register` only if
  stage 2 truly needs one (ask the caller).
- `crate::rad::has_effect(host, &Pl, EffectKey) -> bool`, `rad::hurt(host, EntityRef, f32)` (radiation damage,
  world border's as the fallback), `rad::mobs(host, level) -> Vec<(EntityRef, [f64; 3])>` (`native_mobs`).

Rules of the road:

- Never hold the state's guard (or any of your own `Mutex`es) across a host call that can fire this mod's hooks:
  `run`, `set_block`/`set_air`, `edit_blocks`, teleports, `summon`, `start_bomb`. Take what you need, `drop(guard)`,
  then act. Hooks that may fire inside (`block_changed`, `player_died`, `chunk_load`) only queue.
- `bunker::block_changed`, `bunker::walls_for`, `bunker::take_heard` and `events::mutants_near` run with the state
  locked: they must not call `core::state`.
- Full ids (`big_nukes:…`) everywhere; text colours ARGB; the content is in `assets/` and `data/` of this package.
- `Screen` literals need `recipe_book: None, recipe_book_toggle: None`.
- No toasts in the API: the silo's notices are drawn in `silo::gui_render_post`.
- No `container_accepts` hook: hoppers put into a container's `inputs` by face; the menu's slot rules
  (`machines::Menus`) cover clicks only.

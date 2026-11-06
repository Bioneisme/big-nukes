# `/damage` refuses a content mod's damage type

**Horizon:** Nightly 2026-11-05, `66f3981f522b24c250b21523a70080965b940424` (`NATIVE_HOOKS_API` 411).

## Summary

A damage type from a content mod (`data/<namespace>/damage_type/<name>.json`) is loaded into the game's damage catalog,
but `/damage` doesn't accept its id. The command checks the argument against the command registry exported from vanilla.
`mod_content.rs` adds a mod's items, blocks, effects and entity types to that registry, but not its damage types.

The type itself works: when its id is added to the command registry, `/damage` applies it, and the death message is
the mod's `death.attack.<message_id>`. Only the argument check refuses it.

## Steps to reproduce

1. Make a content mod with namespace `example` and these files:
   - `data/example/damage_type/test.json`:
     ```json
     { "message_id": "example.test", "scaling": "never", "exhaustion": 0.0 }
     ```
   - `assets/example/lang/en_us.json`:
     ```json
     { "death.attack.example.test": "%1$s was tested" }
     ```
2. Open a world with commands allowed.
3. Run `/damage @s 1 example:test`.

**Expected:** 1 damage of type `example:test`. A lethal amount gives the death message "… was tested".

**Actual:** the command is refused with `argument.resource.not_found` for `example:test` and
`minecraft:damage_type`. Vanilla ids such as `/damage @s 1 minecraft:magic` work.

## Cause

- `/damage` parses its type as `ArgType::Res("minecraft:damage_type")` (`crates/horizon-command/src/tree_world.rs:142`).
  The argument is checked with `reg.ids(registry).contains(&id)` (`crates/horizon-command/src/args.rs:1228`).
- `Registries::ids` returns `by_registry[registry].ids` whenever that key exists (`crates/horizon-client-core/src/registries.rs:279-282`).
  The `damage_types` field is consulted only when the key is absent.
- `by_registry["minecraft:damage_type"]` comes from the exported `command-registries.json`, which holds vanilla's ids only.
  `fill_from_exported` doesn't overwrite a key that is already there (`registries.rs:193-194`, `registries.rs:259-262`).
- When content is built, `command-registries.json` gets the mod's ids for `minecraft:item`, `minecraft:block`,
  `minecraft:mob_effect` and `minecraft:entity_type` (`crates/horizon-client/src/mod_content.rs:1606-1640`).
  It doesn't get them for `minecraft:damage_type`.
- `build_registries` fills `registries.damage_types` from `health_resources.damage`, which includes the mod's types
  (`crates/horizon-client/src/game/chat.rs:551-556`). But `ids()` never reaches that list, because the `by_registry`
  entry exists.

The same applies to damage types from a world's data packs, since they come through the same catalog.

## Suggested fix

In `build_registries`, after `damage_types` is filled, set the command registry's ids from it, as `sync_dialog_ids`
does for dialogs (`crates/horizon-client/src/game/lan.rs:165-176`):

```rust
let damage_types = self.ui.registries.damage_types.clone();
self.ui
    .registries
    .by_registry
    .entry("minecraft:damage_type".to_owned())
    .or_default()
    .ids = damage_types;
```

Another option is to add the content mod's damage types to `by_registry/minecraft:damage_type/ids` in `mod_content.rs`,
next to the effects and entity types. That covers content mods but not world data packs.

## Workaround in a native mod

Big Nukes adds its id to `game.ui.registries.by_registry["minecraft:damage_type"]` from `native_server_tick`. It does
this only when its assets half (which brings the type) is installed. This works on `66f3981`: `/damage @s 1000
big_nukes:radiation` kills with "… died of radiation sickness". But `Game.ui` is not an API, so the mod would rather
drop the workaround once the fix lands.

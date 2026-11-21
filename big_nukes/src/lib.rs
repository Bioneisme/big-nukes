//! Big Nukes in the game.
mod rules;

use mod_api::Hooks;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}

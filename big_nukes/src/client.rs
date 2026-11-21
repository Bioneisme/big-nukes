//! The blasts and the radiation in the window. The window hears of a blast once and works out the rest from its own
//! clock: the sounds are queued at once with their delays, the flash and the shake are drawn from the time since
//! the blast and the camera's distance to it. The server tells the window its zones, the mod's clock and the dose
//! once a second (`rad_news`); between the messages the window counts on with its own game time. The Geiger
//! counter in either hand clicks and shows the reading over the hotbar; in a zone the fog turns green and closes in.

use crate::core::{self, argb, lock};
use crate::{keys, rules, sky};
use mod_api::{
    CameraShakeEvent, ClientHost, EquipmentSlot, GuiLayerPostEvent, GuiRenderPostEvent, RenderFogEvent, SoundPlay,
    SoundSource, client::fog::FogType,
};
use std::{
    collections::BTreeSet,
    sync::Mutex,
    time::Instant,
};

/// The eyes over the feet, as the old glue read them: the player's eyes stand for the camera.
const EYES: f64 = 1.62;

/// The local player's dimension and feet.
pub(crate) fn me(host: &dyn ClientHost) -> Option<(String, [f64; 3])> {
    let player = host.local_player()?;
    let dimension = core::identifier(&player.dimension(host)?);
    Some((dimension, player.entity().position(host)?))
}

fn eyes(at: [f64; 3]) -> [f64; 3] {
    [at[0], at[1] + EYES, at[2]]
}

fn play(host: &dyn ClientHost, event: &str, source: SoundSource, at: [f64; 3], volume: f32, pitch: f32, after: u32) {
    let sound = SoundPlay::at(event, source, at).with_volume(volume).with_pitch(pitch);
    let _ = host.play_local_sound(&sound, after);
}

// ---- the blasts ------------------------------------------------------------------

static HEARD: Mutex<Vec<rules::Heard>> = Mutex::new(Vec::new());
/// The blasts whose ringing this window has already heard.
static RANG: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());

/// The news of a blast: its sounds queued from a little toward it, and its time kept for the flash and the shake.
pub(crate) fn heard(host: &mut dyn ClientHost, data: &[u8]) {
    let (Ok(news), Some((dimension, feet)), Some(clock)) = (serde_json::from_slice::<rules::News>(data), me(host), host.clock()) else {
        return;
    };
    let toward = news.center.map(|v| v as f64 + 0.5);
    let far = core::distance(toward, feet);
    log::info!("[big_nukes] blast {} heard at {far:.0} blocks", news.id);
    // From a little toward the blast, so it is heard from where it is.
    let step = if far > 6.0 { 6.0 / far } else { 1.0 };
    let at = [0, 1, 2].map(|i| feet[i] + (toward[i] - feet[i]) * step);
    for s in rules::sounds_for(far, news.radius, news.age) {
        play(host, s.event, SoundSource::Blocks, at, s.volume, 1.0, s.delay.max(0) as u32);
    }
    let mut heard = lock(&HEARD);
    heard.retain(|h| h.news.id != news.id);
    heard.push(rules::Heard { at: clock.game_time as f64 - news.age as f64, dimension, news });
}

/// Each heard blast still felt, with the ticks since it went off and the player's distance to it. `partial` is the
/// part of the tick gone, where the hook knows it.
fn felt(host: &dyn ClientHost, partial: f64) -> Vec<(f64, f64, rules::News)> {
    let (Some(clock), Some((dimension, feet))) = (host.clock(), me(host)) else {
        return vec![];
    };
    let eyes = eyes(feet);
    let now = clock.game_time as f64 + partial;
    let mut heard = lock(&HEARD);
    heard.retain(|h| h.felt(&dimension, eyes, now).is_some() || h.dimension != dimension);
    heard.iter().filter_map(|h| h.felt(&dimension, eyes, now).map(|(t, d)| (t, d, h.news.clone()))).collect()
}

/// The flash, over everything on the screen, even under F1.
pub(crate) fn flash(host: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
    let alpha: u32 = felt(host, 0.0).iter().map(|(t, d, n)| rules::flash_alpha(*t, *d, n.radius) as u32).sum::<u32>().min(255);
    if alpha > 0 {
        let [w, h] = event.batch.size();
        event.batch.fill([0.0, 0.0, w, h], alpha << 24 | 0x00FF_FFFF);
    }
}

/// The ears ring as a close blast's front arrives: the game's sounds muffled and dulled for a while, the harder
/// and the longer the closer (`rules::ringing`), with a high ringing heard clearly through it.
fn ring(host: &mut dyn ClientHost, felt: &[(f64, f64, rules::News)]) {
    let Some((_, feet)) = me(host) else { return };
    let eyes = eyes(feet);
    let mut rang = lock(&RANG);
    for (t, d, news) in felt {
        if !rules::rings_now(*t, *d) || rang.contains(&news.id) {
            continue;
        }
        rang.insert(news.id);
        if let Some(r) = rules::ringing(*d, news.radius)
            && host.muffle_sounds(r.amount, r.lowpass, r.ticks, &[rules::EAR_RINGING])
        {
            play(host, rules::EAR_RINGING, SoundSource::Players, eyes, r.volume, 1.0, 0);
            log::info!("[big_nukes] blast {} rings the ears: {:.2} muffled for {} ticks", news.id, r.amount, r.ticks);
        }
    }
    // Forgotten once no blast it rang for is felt.
    rang.retain(|id| felt.iter().any(|(_, _, n)| n.id == *id));
}

/// Every drawn frame, whatever the screen (death, inventory, chat): the ringing, and the renderer's list of
/// blasts, newest first (`sky::SKY`).
pub(crate) fn frame(host: &mut dyn ClientHost) {
    let partial = host.camera().map_or(0.0, |c| c.partial_tick as f64);
    let mut felt = felt(host, partial);
    rules::newest_first(&mut felt);
    ring(host, &felt);
    *lock(&sky::SKY) = felt
        .iter()
        .map(|(t, _, n)| {
            let c = rules::cloud_of(*t, n);
            sky::SkyBlast {
                center: [n.center[0] as f64 + 0.5, n.center[1] as f64, n.center[2] as f64 + 0.5],
                seed: n.seed as u32,
                age: *t as f32,
                cloud: [c.ball_h, c.ball_r, c.stem_r, c.cap_major, c.cap_minor, c.fire, c.fade, c.ring, c.ring_fade],
            }
        })
        .collect();
}

/// The ground shaking as the front passes.
pub(crate) fn shake(host: &dyn ClientHost, event: &mut CameraShakeEvent<'_>) {
    for (t, d, n) in felt(host, event.partial as f64) {
        let [pitch, yaw, roll, x, y, z] = rules::shake(t, d, n.radius, n.seed);
        let shake = &mut *event.shake;
        shake.pitch += pitch;
        shake.yaw += yaw;
        shake.roll += roll;
        shake.offset[0] += x;
        shake.offset[1] += y;
        shake.offset[2] += z;
    }
}

// ---- the radiation ---------------------------------------------------------------

/// What the window was last told of the radiation.
static TOLD: Mutex<Option<rules::Told>> = Mutex::new(None);
/// The window's ticks, for the clicks' rolls, and the game time of the last one: a clock that stands (single
/// player paused) doesn't click.
static CLICKS: Mutex<(u64, i64)> = Mutex::new((0, i64::MIN));
/// The zone's fog as shown, and when: it eases toward the background's, so crossing into a zone or out of one
/// doesn't jump.
static FOG: Mutex<Option<(Instant, f64)>> = Mutex::new(None);

/// The radiation's message, in the window. One with nothing to tell lets go.
pub(crate) fn told(host: &mut dyn ClientHost, data: &[u8]) {
    let Some(clock) = host.clock() else { return };
    match serde_json::from_slice::<rules::Rad>(data) {
        Ok(rad) => *lock(&TOLD) = (!rad.is_empty()).then(|| rules::Told { rad, at: clock_ticks(clock.game_time) }),
        Err(e) => log::warn!("[big_nukes] the radiation's message: {e}"),
    }
}

/// The window's level goes (another dimension, or the world left): what it was told goes with it. The next
/// message comes within a second.
pub(crate) fn forget_rad() {
    *lock(&TOLD) = None;
}

/// The world closing.
pub(crate) fn forget() {
    lock(&HEARD).clear();
    forget_rad();
}

/// A game time as the rules count it: never below 0.
fn clock_ticks(game_time: i64) -> u64 {
    game_time.max(0) as u64
}

/// The background at `pos` in the window's dimension, by what it was told.
fn background_here(host: &dyn ClientHost, pos: [f64; 3]) -> f64 {
    let (Some((dimension, _)), Some(clock)) = (me(host), host.clock()) else {
        return 0.0;
    };
    lock(&TOLD).as_ref().map_or(0.0, |t| t.background(&dimension, pos, clock_ticks(clock.game_time)))
}

/// The Geiger counter's reading when the player holds one in either hand: the background at their eyes, their dose
/// and where the eyes are.
fn reading(host: &dyn ClientHost) -> Option<(f64, f64, [f64; 3])> {
    let player = host.local_player()?;
    let living = host.living(player.entity())?;
    let holds = [EquipmentSlot::MainHand, EquipmentSlot::OffHand]
        .into_iter()
        .any(|slot| living.equipment(host, slot).is_some_and(|s| s.item() == keys().geiger));
    if !holds {
        return None;
    }
    let (_, feet) = me(host)?;
    let eyes = eyes(feet);
    let dose = lock(&TOLD).as_ref().map_or(0.0, |t| t.rad.dose);
    Some((background_here(host, eyes), dose, eyes))
}

/// The window's tick: the counter clicks by the background, at most once a tick.
pub(crate) fn client_tick(host: &mut dyn ClientHost) {
    let Some(clock) = host.clock() else { return };
    let tick = {
        let mut clicks = lock(&CLICKS);
        let standing = clicks.1 == clock.game_time;
        clicks.0 += 1;
        clicks.1 = clock.game_time;
        if standing {
            return;
        }
        clicks.0
    };
    let Some((background, _, eyes)) = reading(host) else { return };
    if rules::clicks(background, tick) {
        play(host, rules::GEIGER_CLICK, SoundSource::Players, eyes, 0.6, rules::click_pitch(tick), 0);
    }
}

/// The reading's line, this many GUI pixels over the bottom: above the hotbar, the health and food rows, the
/// item's name and the action bar.
const READING_Y: f32 = 84.0;

/// The counter's reading over the hotbar, coloured by the danger. With the HUD: F1 hides it, and a screen covers it.
pub(crate) fn hud(host: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
    if event.layer != "minecraft:hotbar" {
        return;
    }
    let Some((background, dose, _)) = reading(host) else { return };
    let (bg, dose_text) = rules::reading_numbers(background, dose);
    let line = core::lang_tr(host, "big_nukes.geiger.reading", &[&bg, &dose_text]);
    let draw = &mut *event.draw;
    let [w, h] = draw.size();
    let x = ((w - draw.text_width(&line)) / 2.0).round();
    draw.text(&line, [x, h - READING_Y], argb(rules::reading_colour(background)), true);
}

/// The world's fog, every frame: a zone's turns it green and brings it in, by the background at the camera, eased
/// over time. Only the fog of the open air: water, lava and powder snow keep their own.
pub(crate) fn fog(host: &dyn ClientHost, event: &mut RenderFogEvent<'_>) {
    let overworld = me(host).is_some_and(|(d, _)| d == "minecraft:overworld");
    // A storm's green over the overworld, as a strong zone's. Not in a sealed room.
    let storm = overworld && lock(&TOLD).as_ref().is_some_and(|t| t.rad.storm && t.rad.shelter > rules::SEALED_SHELTER);
    let target = rules::fog_strength(background_here(host, event.camera)).max(if storm { rules::STORM_FOG } else { 0.0 });
    let now = Instant::now();
    let shown = {
        let mut eased = lock(&FOG);
        let shown = match *eased {
            Some((then, shown)) => rules::ease(shown, target, now.duration_since(then).as_secs_f64()),
            None => target,
        };
        *eased = Some((now, shown));
        shown
    };
    if event.fog_type != FogType::Atmospheric {
        return;
    }
    // The overworld's fog darkens with the night, and the zone's with it; the other dimensions keep no time.
    let light = match host.clock() {
        Some(clock) if overworld => rules::sky_light(clock.day_time),
        _ => 1.0,
    };
    let fog = &mut *event.fog;
    // A nuclear winter greys the overworld's fog first; a zone's green goes over it.
    let winter = overworld && lock(&TOLD).as_ref().is_some_and(|t| t.rad.winter);
    if winter && let Some((color, start, end)) = rules::winter_fog(fog.color, fog.environmental_start, fog.environmental_end, light) {
        fog.color = color;
        fog.environmental_start = start;
        fog.environmental_end = end;
    }
    if let Some((color, start, end)) = rules::tint_fog(shown, fog.color, fog.environmental_start, fog.environmental_end, light) {
        fog.color = color;
        fog.environmental_start = start;
        fog.environmental_end = end;
    }
}

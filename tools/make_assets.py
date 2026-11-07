#!/usr/bin/env python3
"""Writes the assets half's JSON for the blocks and items: block states, models, item models,
loot tables, tags, the ore and ash declarations, the radiation's mob effect, the suit's equipment asset,
and their names in both languages.

    python tools/make_assets.py

The same input gives the same files; hand-written files (recipes, worldgen) are left alone.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "content"
A = ROOT / "assets" / "big_nukes"
D = ROOT / "data" / "big_nukes"

# id: (kind of model, English, Russian)
FACING = {
    "centrifuge": ("Centrifuge", "Центрифуга"),
    "demolition_charge": ("Demolition Charge", "Подрывной заряд"),
    "tactical_warhead": ("Tactical Warhead", "Тактическая боеголовка"),
    "atomic_bomb": ("Atomic Bomb", "Атомная бомба"),
    "launch_pad": ("Launch Pad", "Пусковой стол"),
    "launch_console": ("Launch Console", "Пульт запуска"),
    "strike_beacon": ("Strike Beacon", "Маяк авиаудара"),
}
TABLE = {"assembly_table": ("Assembly Table", "Сборочный стол"), "silo_hatch": ("Silo Hatch", "Люк шахты")}
# Declared in files: id: (template, English, Russian, drop)
FILES = {
    "uranium_ore": ("minecraft:iron_ore", "Uranium Ore", "Урановая руда", "big_nukes:raw_uranium"),
    "deepslate_uranium_ore": ("minecraft:deepslate_iron_ore", "Deepslate Uranium Ore", "Глубинная урановая руда", "big_nukes:raw_uranium"),
    "ash": ("minecraft:coarse_dirt", "Ash", "Пепел", "big_nukes:ash"),
}
ITEMS = {
    "raw_uranium": ("Raw Uranium", "Рудный уран"),
    "enriched_uranium": ("Enriched Uranium", "Обогащённый уран"),
    "nuclear_charge": ("Nuclear Charge", "Ядерный заряд"),
    "bomb_casing": ("Bomb Casing", "Корпус бомбы"),
    "fuse_detonator": ("Fuse Detonator", "Фитильный детонатор"),
    "keypad_detonator": ("Keypad Detonator", "Кодовый детонатор"),
    "receiver_detonator": ("Receiver Detonator", "Детонатор-приёмник"),
    "remote": ("Remote Detonator", "Пульт подрыва"),
    "hazmat_helmet": ("Hazmat Helmet", "Шлем химзащиты"),
    "hazmat_chestplate": ("Hazmat Jacket", "Куртка химзащиты"),
    "hazmat_leggings": ("Hazmat Pants", "Штаны химзащиты"),
    "hazmat_boots": ("Hazmat Boots", "Ботинки химзащиты"),
    "anti_rad": ("Anti-Rad", "Антирад"),
    "geiger_counter": ("Geiger Counter", "Счётчик Гейгера"),
    "missile": ("Missile", "Ракета"),
    "guide": ("Big Nukes Guide", "Руководство Big Nukes"),
}


# The 3D models the flying displays show, drawn here along z with the nose at +z: (from, to, region of the 16x16
# sheet). They are written turned round (nose at -z): an item display draws its item half a turn about y, so the
# nose comes out along the display's look.
def cube(frm, to, region):
    frm, to = [frm[0], frm[1], 16 - to[2]], [to[0], to[1], 16 - frm[2]]
    return {"from": frm, "to": to, "faces": {f: {"uv": region, "texture": "#t"} for f in ("north", "south", "east", "west", "up", "down")}}


WHITE, RED, DARK, BLACK = [0, 0, 8, 8], [8, 0, 16, 8], [0, 8, 8, 16], [8, 8, 16, 16]
# 1 x 6 blocks at the display's scale of 2: a white body, a red band, nose and fins, a dark nozzle.
MISSILE_MODEL = [
    cube([4, 4, -12], [12, 12, 22], WHITE),
    cube([3.8, 3.8, 14], [12.2, 12.2, 16], RED),
    cube([5, 5, 22], [11, 11, 27], RED),
    cube([6.5, 6.5, 27], [9.5, 9.5, 31], RED),
    cube([5, 5, -16], [11, 11, -12], DARK),
    cube([7.25, 12, -16], [8.75, 16, -7], RED),
    cube([7.25, 0, -16], [8.75, 4, -7], RED),
    cube([0, 7.25, -16], [4, 8.75, -7], RED),
    cube([12, 7.25, -16], [16, 8.75, -7], RED),
]
METAL, LIGHT, GLASS, PROP = [0, 0, 8, 8], [8, 0, 16, 8], [0, 8, 8, 16], [8, 8, 16, 16]
# 12 blocks of wing at the display's scale of 4: a dark fuselage, four engines, a glazed nose and a tail.
BOMBER_MODEL = [
    cube([6, 5, -14], [10, 9, 30], METAL),
    cube([6.5, 5.5, 30], [9.5, 8.5, 32], GLASS),
    cube([6.5, 9, 23], [9.5, 10.5, 29], GLASS),
    cube([-16, 6.5, 6], [32, 7.5, 14], LIGHT),
    cube([1, 7, -14], [15, 8, -9], LIGHT),
    cube([7.5, 9, -14], [8.5, 14, -8], METAL),
] + [cube([x, 5, 8], [x + 3, 8, 16], METAL) for x in (-11, -3, 16, 24)]   + [cube([x - 1, 4, 16], [x + 4, 9, 16.5], PROP) for x in (-11, -3, 16, 24)]
MODELS = {
    "missile_model": (MISSILE_MODEL, "Missile in Flight", "Ракета в полёте"),
    "bomber_model": (BOMBER_MODEL, "Bomber", "Бомбардировщик"),
}
# Mob effects: id: (category, RGB, English, Russian)
EFFECTS = {"radiation": ("harmful", 0x7FB238, "Radiation", "Радиация")}


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def loot(name, drop):
    write(D / "loot_table" / "blocks" / f"{name}.json", {
        "type": "minecraft:block",
        "pools": [{"condition": {"type": "minecraft:survives_explosion"}, "entries": [{"type": "minecraft:item", "name": drop}], "rolls": 1}],
        "random_sequence": f"big_nukes:blocks/{name}",
    })


def block_item(name):
    write(A / "items" / f"{name}.json", {"model": {"type": "minecraft:model", "model": f"big_nukes:block/{name}"}})


lang = {"en_us": {}, "ru_ru": {}}
for name, (en, ru) in FACING.items():
    write(A / "blockstates" / f"{name}.json", {"variants": {
        f"facing={f}": ({"model": f"big_nukes:block/{name}"} | ({"y": y} if y else {}))
        for f, y in (("north", 0), ("east", 90), ("south", 180), ("west", 270))}})
    write(A / "models" / "block" / f"{name}.json", {"parent": "minecraft:block/cube", "textures": {
        "particle": f"big_nukes:block/{name}_side", "down": f"big_nukes:block/{name}_top", "up": f"big_nukes:block/{name}_top",
        "north": f"big_nukes:block/{name}_front", "south": f"big_nukes:block/{name}_side",
        "west": f"big_nukes:block/{name}_side", "east": f"big_nukes:block/{name}_side"}})
    block_item(name)
    loot(name, f"big_nukes:{name}")
    lang["en_us"][f"block.big_nukes.{name}"] = en
    lang["ru_ru"][f"block.big_nukes.{name}"] = ru
for name, (en, ru) in TABLE.items():
    write(A / "blockstates" / f"{name}.json", {"variants": {"": {"model": f"big_nukes:block/{name}"}}})
    write(A / "models" / "block" / f"{name}.json", {"parent": "minecraft:block/cube_bottom_top", "textures": {
        "top": f"big_nukes:block/{name}_top", "bottom": f"big_nukes:block/{name}_bottom", "side": f"big_nukes:block/{name}_side"}})
    block_item(name)
    loot(name, f"big_nukes:{name}")
    lang["en_us"][f"block.big_nukes.{name}"] = en
    lang["ru_ru"][f"block.big_nukes.{name}"] = ru
for name, (template, en, ru, drop) in FILES.items():
    write(D / "horizon" / "block" / f"{name}.json", {"template": template})
    write(A / "blockstates" / f"{name}.json", {"variants": {"": {"model": f"big_nukes:block/{name}"}}})
    write(A / "models" / "block" / f"{name}.json", {"parent": "minecraft:block/cube_all", "textures": {"all": f"big_nukes:block/{name}"}})
    block_item(name)
    loot(name, drop)
    lang["en_us"][f"block.big_nukes.{name}"] = en
    lang["ru_ru"][f"block.big_nukes.{name}"] = ru
for name, (en, ru) in ITEMS.items():
    write(A / "items" / f"{name}.json", {"model": {"type": "minecraft:model", "model": f"big_nukes:item/{name}"}})
    write(A / "models" / "item" / f"{name}.json", {"parent": "minecraft:item/generated", "textures": {"layer0": f"big_nukes:item/{name}"}})
    lang["en_us"][f"item.big_nukes.{name}"] = en
    lang["ru_ru"][f"item.big_nukes.{name}"] = ru

for name, (elements, en, ru) in MODELS.items():
    write(A / "items" / f"{name}.json", {"model": {"type": "minecraft:model", "model": f"big_nukes:item/{name}"}})
    write(A / "models" / "item" / f"{name}.json", {
        "textures": {"t": f"big_nukes:item/{name}", "particle": f"big_nukes:item/{name}"}, "elements": elements})
    lang["en_us"][f"item.big_nukes.{name}"] = en
    lang["ru_ru"][f"item.big_nukes.{name}"] = ru

for name, (category, color, en, ru) in EFFECTS.items():
    write(D / "horizon" / "mob_effect" / f"{name}.json", {"category": category, "color": color})
    lang["en_us"][f"effect.big_nukes.{name}"] = en
    lang["ru_ru"][f"effect.big_nukes.{name}"] = ru
# The suit on a player: its pieces' layers, drawn by make_textures.py.
write(A / "equipment" / "hazmat.json", {"layers": {
    "humanoid": [{"texture": "big_nukes:hazmat"}], "humanoid_leggings": [{"texture": "big_nukes:hazmat"}]}})


# The blocks declared in code are there only with the native half: their entries are optional,
# so the assets half alone still loads its tags (and with them its data).
def entry(n):
    return {"id": f"big_nukes:{n}", "required": False} if n in FACING or n in TABLE else f"big_nukes:{n}"


pickaxe = [entry(n) for n in [*FACING, *TABLE, "uranium_ore", "deepslate_uranium_ore"]]
write(ROOT / "data" / "minecraft" / "tags" / "block" / "mineable" / "pickaxe.json", {"values": pickaxe})
write(ROOT / "data" / "minecraft" / "tags" / "block" / "mineable" / "shovel.json", {"values": ["big_nukes:ash"]})
write(ROOT / "data" / "minecraft" / "tags" / "block" / "needs_iron_tool.json",
      {"values": [entry("uranium_ore"), entry("deepslate_uranium_ore"), entry("centrifuge")]})

for code, add in lang.items():
    path = A / "lang" / f"{code}.json"
    current = json.loads(path.read_text(encoding="utf-8"))
    current.update(add)
    path.write_text(json.dumps(current, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print("assets written")

#!/usr/bin/env python3
"""Writes the assets half's JSON for the blocks and items: block states, models, item models,
loot tables, tags, the ore and ash declarations, and their names in both languages.

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
}
TABLE = {"assembly_table": ("Assembly Table", "Сборочный стол")}
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
}


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

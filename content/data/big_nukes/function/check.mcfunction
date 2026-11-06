# Tell each player, once, when the native half of the mod isn't there: that half
# tags every player as they join, and the tag is missing without it.
execute as @a[tag=!big_nukes.native,tag=!big_nukes.told] run tellraw @s {"translate":"big_nukes.missing.native","color":"red","extra":[{"text":" "},{"text":"https://playhorizon.dev/mods/big_nukes_native","color":"aqua","underlined":true,"click_event":{"action":"open_url","url":"https://playhorizon.dev/mods/big_nukes_native"}}]}
tag @a[tag=!big_nukes.native,tag=!big_nukes.told] add big_nukes.told

# Sources and status

Accuracy tracking: where each behaviour came from, and how sure we are. Keep the three status
columns separate: **implemented** (code exists), **tested** (a test pins it), **compared** (checked
side by side with the original game). One never implies another.

| Behaviour | Source (file + field, exe + address, or measurement) | Implemented | Tested | Compared |
|---|---|---|---|---|
| Run speed | extracted::RUN_SPEED | yes | `run_speed_matches_the_data` | no |
| Jump height | extracted::JUMP_HEIGHT; launch = sqrt(2 g h) | yes | `jump_reaches_the_data_height` | no |
| Roll i-frames / distance | ACTIONS["Roll"] | yes | `roll_iframes_match_the_frame_data`, `roll_travels_its_root_motion` | no |
| Attack chain / buffering | ACTIONS["Light1"].input_from / cancel_into_attack | yes | `buffered_attack_chains_exactly_at_the_cancel_frame` | no |
| Survival content (items, recipes, nodes, wolves, tunables) | data/content.ron, all ESTIMATE (original game) | yes | `content_loads_and_every_reference_resolves` | n/a |
| Gathering, best tool, yield, respawn | content nodes + `Sim::hit_node` | yes | `punching_a_tree_*`, `an_axe_raises_the_yield`, `a_node_depletes_*`, `iron_needs_*` | n/a |
| Crafting, stations, skill gates | content recipes + `Sim::craft` | yes | `crafting_checks_*`, `a_station_must_be_near`, `smelting_at_the_furnace` | n/a |
| Hunger, death, respawn | tunables + `step_vitals` / `step_dead` | yes | `hunger_drains_*`, `starving_to_death_*` | n/a |
| Day/night, wolf spawns | tunables + `step_clock` | yes | `night_falls_on_schedule_*`, `later_nights_bring_more_wolves` | n/a |
| Wolf AI, telegraphed bite, dodge | enemies + `step_wolves` | yes | `a_wolf_chases_telegraphs_and_bites`, `rolling_through_the_bite_dodges_it` | n/a |
| Beacon victory, full loop | `Sim::interact`, `sim::bot` | yes | `lighting_the_beacon_*`, `scripted_playthrough_lights_the_beacon`, `the_whole_playthrough_replays_identically` | n/a |

## Estimates
Each ESTIMATE: why it is not extracted yet, and what would resolve it (which file, which function).

## Research notes
Addresses with the exe build/hash, decoded constants, formulas, and branch conditions. Raw
decompiler output stays in the private research folder (`%USERPROFILE%\re-private\<game>`) so the
repo stays small and readable.

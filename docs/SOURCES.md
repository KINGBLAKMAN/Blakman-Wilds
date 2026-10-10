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

## Estimates
Each ESTIMATE: why it is not extracted yet, and what would resolve it (which file, which function).

## Research notes
Addresses with the exe build/hash, decoded constants, formulas, and branch conditions. Raw
decompiler output stays in the private research folder (`%USERPROFILE%\re-private\<game>`) so the
repo stays small and readable.

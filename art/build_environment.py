"""Emberwild environment assets: trees, rocks, ore, bush, stations, beacon.
Run inside Blender after asset_kit.py:  exec(open(r"...\build_environment.py").read())
Exports one .glb per asset to OUT and lays them out in a showcase row.
"""
import math
import os

OUT = r"C:\Users\tankl\OneDrive\Desktop\ThingsClaudeCanTouch\emberwild\assets\models"
os.makedirs(OUT, exist_ok=True)


def tree_pine(name="tree_pine", seed=1):
    c = collection(name)
    parts = [cone(f"{name}_trunk", c, "bark", 0.22, 0.16, 1.4, segments=6)]
    z, r = 1.0, 1.35
    for i in range(3):
        parts.append(cone(f"{name}_tier{i}", c, ["leaf_dark", "leaf", "leaf_light"][i], r, 0.05, 1.6, segments=7, loc=(0, 0, z)))
        z += 0.95
        r *= 0.72
    return join(parts, name)


def tree_round(name="tree_round", seed=2):
    c = collection(name)
    parts = [cone(f"{name}_trunk", c, "bark", 0.26, 0.18, 2.0, segments=6)]
    parts.append(blob(f"{name}_crown0", c, "leaf", 1.25, loc=(0, 0, 2.6), scale=(1, 1, 0.85), seed=seed))
    parts.append(blob(f"{name}_crown1", c, "leaf_light", 0.8, loc=(0.6, 0.3, 3.2), scale=(1, 1, 0.9), seed=seed + 1))
    parts.append(blob(f"{name}_crown2", c, "leaf_dark", 0.85, loc=(-0.6, -0.2, 2.3), scale=(1, 1, 0.9), seed=seed + 2))
    return join(parts, name)


def stump(name="tree_stump"):
    c = collection(name)
    return join([cone(f"{name}_s", c, "bark", 0.26, 0.24, 0.35, segments=6),
                 cone(f"{name}_top", c, "wood", 0.2, 0.2, 0.02, segments=6, loc=(0, 0, 0.35))], name)


def rock(name="rock", seed=3):
    c = collection(name)
    return join([blob(f"{name}_a", c, "stone", 0.75, loc=(0, 0, 0.35), scale=(1.2, 1.0, 0.7), seed=seed, jitter=0.18),
                 blob(f"{name}_b", c, "stone_dark", 0.45, loc=(0.6, 0.3, 0.2), scale=(1, 1, 0.8), seed=seed + 5, jitter=0.2)], name)


def iron_rock(name="iron_rock", seed=7):
    c = collection(name)
    parts = [blob(f"{name}_a", c, "stone_dark", 0.8, loc=(0, 0, 0.4), scale=(1.1, 1.0, 0.75), seed=seed, jitter=0.18)]
    for i, (x, y, z) in enumerate([(0.35, 0.5, 0.65), (-0.45, 0.35, 0.55), (0.1, -0.55, 0.6), (-0.2, 0.0, 0.95)]):
        parts.append(blob(f"{name}_ore{i}", c, "iron", 0.18, loc=(x, y, z), seed=seed + 10 + i, subdiv=0, jitter=0.1))
    return join(parts, name)


def berry_bush(name="berry_bush", seed=11):
    c = collection(name)
    parts = [blob(f"{name}_b0", c, "leaf", 0.6, loc=(0, 0, 0.45), scale=(1.2, 1.1, 0.8), seed=seed),
             blob(f"{name}_b1", c, "leaf_dark", 0.45, loc=(0.4, -0.2, 0.35), seed=seed + 1)]
    for i, (x, y, z) in enumerate([(0.35, 0.45, 0.6), (-0.5, 0.2, 0.55), (0.1, -0.5, 0.7), (-0.2, -0.3, 0.85), (0.55, 0.0, 0.75)]):
        parts.append(blob(f"{name}_berry{i}", c, "berry", 0.08, loc=(x, y, z), subdiv=1, jitter=0.0, seed=i))
    return join(parts, name)


def campfire(name="campfire"):
    c = collection(name)
    parts = []
    for i in range(7):
        a = i / 7 * math.tau
        parts.append(blob(f"{name}_stone{i}", c, "stone", 0.14, loc=(math.cos(a) * 0.55, math.sin(a) * 0.55, 0.08), seed=i, subdiv=0))
    for i in range(3):
        a = i / 3 * math.tau
        parts.append(cone(f"{name}_log{i}", c, "bark", 0.07, 0.06, 0.9, segments=5,
                          loc=(math.cos(a) * 0.3, math.sin(a) * 0.3, 0.05), rot=(0, math.radians(65), a + math.pi)))
    parts.append(cone(f"{name}_flame", c, "ember", 0.25, 0.02, 0.6, segments=5, loc=(0, 0, 0.1)))
    return join(parts, name)


def workbench(name="workbench"):
    c = collection(name)
    parts = [box(f"{name}_top", c, "wood", (1.6, 0.8, 0.12), loc=(0, 0, 0.8), bevel=0.02)]
    for x in (-0.7, 0.7):
        for y in (-0.32, 0.32):
            parts.append(box(f"{name}_leg{x}{y}", c, "bark", (0.1, 0.1, 0.8), loc=(x, y, 0)))
    parts.append(box(f"{name}_saw", c, "metal", (0.5, 0.02, 0.15), loc=(0.3, 0.1, 0.92)))
    parts.append(box(f"{name}_block", c, "bark_dark", (0.3, 0.3, 0.2), loc=(-0.4, 0.0, 0.92)))
    return join(parts, name)


def furnace(name="furnace"):
    c = collection(name)
    parts = [cone(f"{name}_body", c, "stone", 0.75, 0.55, 1.3, segments=8),
             cone(f"{name}_chimney", c, "stone_dark", 0.22, 0.2, 0.7, segments=6, loc=(0, 0, 1.3)),
             box(f"{name}_mouth", c, "ember", (0.5, 0.15, 0.35), loc=(0, -0.62, 0.15))]
    return join(parts, name)


def beacon(name="beacon"):
    c = collection(name)
    parts = [cone(f"{name}_base", c, "stone_dark", 1.6, 1.3, 0.5, segments=8),
             cone(f"{name}_step", c, "stone", 1.2, 1.0, 0.4, segments=8, loc=(0, 0, 0.5)),
             cone(f"{name}_pillar", c, "stone", 0.45, 0.35, 4.0, segments=8, loc=(0, 0, 0.9)),
             cone(f"{name}_bowl", c, "metal", 0.4, 0.9, 0.5, segments=8, loc=(0, 0, 4.9))]
    for i in range(4):
        a = i / 4 * math.tau + math.pi / 4
        parts.append(box(f"{name}_rune{i}", c, "iron", (0.15, 0.15, 1.2), loc=(math.cos(a) * 1.05, math.sin(a) * 1.05, 0.5)))
    return join(parts, name)


def beacon_flame(name="beacon_flame"):
    c = collection(name)
    return join([cone(f"{name}_f0", c, "ember", 0.6, 0.02, 1.4, segments=6),
                 cone(f"{name}_f1", c, "ember", 0.35, 0.02, 1.9, segments=5, loc=(0.15, 0.1, 0))], name)


BUILDERS = [tree_pine, tree_round, stump, rock, iron_rock, berry_bush, campfire, workbench, furnace, beacon, beacon_flame]


def build_all(showcase=True):
    report = []
    x = 0.0
    for build in BUILDERS:
        obj = build()
        name = obj.name
        path = export_asset(name, os.path.join(OUT, f"{name}.glb"))
        report.append((name, stats(name), os.path.getsize(path)))
        if showcase:
            obj.location.x = x
            x += 4.0
    return report

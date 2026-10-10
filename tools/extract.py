"""Build generated/extracted.rs from data/measurements.json.

    python tools/extract.py            (or python tools/setup.py)

Dragonwilds' asset container is encrypted, so values come from measurement of gameplay clips
(MEASURED) or start as tuned guesses (ESTIMATE). Each constant in the generated table carries its tag.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import emit
import paths
from emit import Value

MEASUREMENTS = paths.ROOT / "data" / "measurements.json"


def tagged(entry, what):
    note = f"{entry['source']} [{entry.get('unit', 'frames')}]" if "unit" in entry else entry["source"]
    if entry["status"] in ("MEASURED", "PUBLIC"):
        return Value(entry["value"], src=f"{entry['status']} {what}: {note}") if "value" in entry else note
    if entry["status"] == "VIDEO":
        return Value(entry["value"], estimate=f"VIDEO {what}: {note}")
    return Value(entry["value"], estimate=f"{what}: {note}") if "value" in entry else note


def main():
    data = json.loads(MEASUREMENTS.read_text(encoding="utf-8"))
    scalars = {name: tagged(e, name) for name, e in data["scalars"].items()}
    actions = []
    for a in data["actions"]:
        a = dict(a)
        a["src"] = f"{a.pop('status')}: {a.pop('source')}"
        a["hit"] = tuple(a["hit"]) if a.get("hit") else None
        a["iframes"] = tuple(a["iframes"]) if a.get("iframes") else None
        actions.append(a)
    known = sum(1 for e in data["scalars"].values() if e["status"] in ("MEASURED", "PUBLIC"))
    video = sum(1 for e in data["scalars"].values() if e["status"] == "VIDEO")
    source = f"Dragonwilds data ({known}/{len(scalars)} public or measured, {video} from video)"
    emit.write(paths.generated_dir() / "extracted.rs", source, scalars, actions)


if __name__ == "__main__":
    main()

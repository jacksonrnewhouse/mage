#!/usr/bin/env python3
"""Fetch oracle text for every card in the card pool from Scryfall API."""

import json
import os
import time
import urllib.request
import urllib.parse
import urllib.error

CARDS_DIR = os.path.dirname(os.path.abspath(__file__))
OUTPUT_FILE = os.path.join(CARDS_DIR, "oracle_text.json")

def get_card_names():
    """Read all card names from cards/*.txt files."""
    names = []
    for filename in sorted(os.listdir(CARDS_DIR)):
        if filename.endswith(".txt"):
            filepath = os.path.join(CARDS_DIR, filename)
            with open(filepath) as f:
                for line in f:
                    line = line.strip()
                    if line:
                        names.append(line)
    return sorted(set(names))


def fetch_oracle_text(card_name: str) -> dict:
    """Fetch a card's oracle text from Scryfall API."""
    url = "https://api.scryfall.com/cards/named?" + urllib.parse.urlencode({"exact": card_name})
    req = urllib.request.Request(url, headers={
        "User-Agent": "MageEngine/1.0",
        "Accept": "application/json",
    })
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        print(f"  ERROR {e.code}: {card_name}")
        return {"name": card_name, "error": f"HTTP {e.code}"}

    result = {
        "name": data.get("name", card_name),
        "mana_cost": data.get("mana_cost", ""),
        "type_line": data.get("type_line", ""),
    }

    # Double-faced cards have oracle text in card_faces
    if "card_faces" in data:
        faces = []
        for face in data["card_faces"]:
            face_data = {
                "name": face.get("name", ""),
                "mana_cost": face.get("mana_cost", ""),
                "type_line": face.get("type_line", ""),
                "oracle_text": face.get("oracle_text", ""),
            }
            if face.get("power") is not None:
                face_data["power"] = face["power"]
                face_data["toughness"] = face["toughness"]
            if face.get("loyalty") is not None:
                face_data["loyalty"] = face["loyalty"]
            faces.append(face_data)
        result["card_faces"] = faces
    else:
        result["oracle_text"] = data.get("oracle_text", "")

    if data.get("power") is not None:
        result["power"] = data["power"]
        result["toughness"] = data["toughness"]
    if data.get("loyalty") is not None:
        result["loyalty"] = data["loyalty"]
    if data.get("keywords"):
        result["keywords"] = data["keywords"]

    return result


def main():
    card_names = get_card_names()
    print(f"Fetching oracle text for {len(card_names)} cards...")

    # Load existing results to allow resuming
    results = {}
    if os.path.exists(OUTPUT_FILE):
        with open(OUTPUT_FILE) as f:
            existing = json.load(f)
            for entry in existing:
                results[entry["name"]] = entry

    for i, name in enumerate(card_names):
        # Skip if already fetched successfully
        if name in results and "error" not in results[name]:
            print(f"  [{i+1}/{len(card_names)}] {name} (cached)")
            continue

        print(f"  [{i+1}/{len(card_names)}] {name}...")
        result = fetch_oracle_text(name)
        results[result.get("name", name)] = result

        # Scryfall asks for 50-100ms between requests
        time.sleep(0.1)

        # Save periodically
        if (i + 1) % 20 == 0:
            with open(OUTPUT_FILE, "w") as f:
                json.dump(sorted(results.values(), key=lambda x: x["name"]), f, indent=2)

    # Final save
    with open(OUTPUT_FILE, "w") as f:
        json.dump(sorted(results.values(), key=lambda x: x["name"]), f, indent=2)

    errors = [r for r in results.values() if "error" in r]
    print(f"\nDone! {len(results)} cards saved to {OUTPUT_FILE}")
    if errors:
        print(f"  {len(errors)} errors:")
        for e in errors:
            print(f"    - {e['name']}: {e['error']}")


if __name__ == "__main__":
    main()

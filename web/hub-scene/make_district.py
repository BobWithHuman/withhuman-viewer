"""Makes src/admin/district.json, the part of a real town 기록's map is drawn on.

Reads an Overpass API answer (roads and buildings of a small part of Seongsu-dong, Seoul, from
OpenStreetMap) and keeps what the map needs: the roads cars use, by kind, and the buildings'
outlines with a height. The data is (c) OpenStreetMap contributors, under the ODbL; the map
says so in its corner.

    curl --data-urlencode 'data=[out:json][timeout:90];(way["highway"](37.5412,127.0505,37.5478,127.0600);way["building"](37.5412,127.0505,37.5478,127.0600););out body geom;' \
        https://overpass-api.de/api/interpreter -o osm.json
    python3 -I web/hub-scene/make_district.py osm.json src/admin/district.json
"""

import json
import sys

# The roads a route can take, by how wide the map draws them.
KINDS = {
    "trunk": "major", "primary": "major", "secondary": "major", "tertiary": "major",
    "residential": "minor", "unclassified": "minor", "living_street": "minor", "pedestrian": "minor",
    "service": "lane",
}


def main(source, target):
    with open(source, encoding="utf-8") as f:
        elements = json.load(f)["elements"]
    roads, buildings = [], []
    for way in elements:
        tags = way.get("tags", {})
        points = [[round(p["lon"], 6), round(p["lat"], 6)] for p in way.get("geometry", [])]
        if tags.get("highway") in KINDS and len(points) > 1:
            roads.append({"kind": KINDS[tags["highway"]], "points": points})
        elif "building" in tags and len(points) > 3 and points[0] == points[-1]:
            try:
                height = float(tags["height"]) if "height" in tags else float(tags["building:levels"]) * 3.2
            except (KeyError, ValueError):
                # No height recorded: one that is always the same for the same building.
                height = 7.0 + (way["id"] % 5) * 2.4
            buildings.append({"ring": points, "height": round(min(height, 60.0), 1)})
    lons = [p[0] for r in roads for p in r["points"]]
    lats = [p[1] for r in roads for p in r["points"]]
    district = {
        "credit": "© OpenStreetMap contributors",
        "centre": [round((min(lons) + max(lons)) / 2, 6), round((min(lats) + max(lats)) / 2, 6)],
        "roads": roads,
        "buildings": buildings,
    }
    with open(target, "w", encoding="utf-8") as f:
        json.dump(district, f, ensure_ascii=False, separators=(",", ":"))
    print(f"{len(roads)} roads, {len(buildings)} buildings")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

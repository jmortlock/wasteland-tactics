#!/usr/bin/env python3
"""Generates assets/maps/mission01.tmx from the ASCII layout below. Edit in Tiled afterwards if you like."""
ART = """\
####################
#..................#
#......#........E..#
#......#....=...E..#
#......#.......E...#
#..........###.....#
#..|...............#
#..|.......#.......#
#..........#...=...#
#....###...#.......#
#..................#
#.........|........#
#..P.P.............#
#..P.P.............#
####################"""
TILE = 64
GID = {".": 1, "#": 2, "=": 3, "|": 4}  # firstgid 1 + tile id
rows = ART.splitlines()
h, w = len(rows), len(rows[0])
ground = ",".join("1" for _ in range(w * h))
walls = ",".join(str(GID.get(c, 0)) if c in "#=|" else "0" for r in rows for c in r)
objects, oid = [], 1
for y, row in enumerate(rows):
    for x, c in enumerate(row):
        if c in "PE":
            faction = "player" if c == "P" else "enemy"
            objects.append(
                f'  <object id="{oid}" name="{faction}{oid}" x="{x * TILE + TILE // 2}" y="{y * TILE + TILE // 2}">\n'
                f'   <point/>\n   <properties>\n    <property name="faction" value="{faction}"/>\n   </properties>\n  </object>'
            )
            oid += 1
tmx = f"""<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" tiledversion="1.10.2" orientation="orthogonal" renderorder="right-down" width="{w}" height="{h}" tilewidth="{TILE}" tileheight="{TILE}" infinite="0" nextlayerid="4" nextobjectid="{oid}">
 <tileset firstgid="1" source="wasteland.tsx"/>
 <layer id="1" name="ground" width="{w}" height="{h}">
  <data encoding="csv">
{ground}
</data>
 </layer>
 <layer id="2" name="walls" width="{w}" height="{h}">
  <data encoding="csv">
{walls}
</data>
 </layer>
 <objectgroup id="3" name="spawns">
{chr(10).join(objects)}
 </objectgroup>
</map>
"""
open("assets/maps/mission01.tmx", "w").write(tmx)
print(f"wrote {w}x{h} map with {oid - 1} spawns")

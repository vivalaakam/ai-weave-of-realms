#!/usr/bin/env python3
"""Generate mountain tileset 3_mountains.png — 5 tiles (80×16), monochrome with alpha.

Tile layout (same 5-tile compositing system as 2_water.png):
  0  FULL          — all 16×16 pixels set (mountain body)
  1  SHORE_LL      — lower-left half of W cliff edge missing
  2  SHORE_UL      — upper-left half of W cliff edge missing
  3  CORNER_OUTER  — diagonal cliff cut at upper-left (NW outer corner)
  4  CORNER_INNER  — 3-pixel notch at upper-left (NW inner corner)

All edge/corner pieces are for the W / upper-left case; the renderer rotates them
for N, E, S directions and other corners.
"""

from PIL import Image

TILE_W, TILE_H = 16, 16
COLS = 5

def full_pixels():
    return {(x, y) for y in range(TILE_H) for x in range(TILE_W)}

def shore_ll_pixels():
    """Lower half of W cliff edge.

    Rows 0-7:  full mountain body.
    Rows 8-15: stepped cliff — each pair of rows loses one more column from the left.
      row 8-9:   1 pixel missing (x=0)
      row 10-11: 2 pixels missing (x=0..1)
      row 12-13: 3 pixels missing (x=0..2)
      row 14-15: 4 pixels missing (x=0..3)
    """
    pixels = full_pixels()
    for row_pair, depth in enumerate([1, 2, 3, 4]):
        for sub in range(2):
            y = 8 + row_pair * 2 + sub
            for x in range(depth):
                pixels.discard((x, y))
    return pixels

def shore_ul_pixels():
    """Upper half of W cliff edge (mirror of SHORE_LL in vertical).

    Rows 0-7:  stepped cliff — tapers from 4px at top to 1px at bottom.
      row 0-1:  4 pixels missing
      row 2-3:  3 pixels missing
      row 4-5:  2 pixels missing
      row 6-7:  1 pixel missing
    Rows 8-15: full mountain body.
    """
    pixels = full_pixels()
    for row_pair, depth in enumerate([4, 3, 2, 1]):
        for sub in range(2):
            y = row_pair * 2 + sub
            for x in range(depth):
                pixels.discard((x, y))
    return pixels

def corner_outer_pixels():
    """Outer corner at upper-left (NW) — diagonal cliff.

    Covers the area where both W and N edges meet.
    Steps diagonally from wide at the top to narrow at row 7.
      row 0: 8 missing
      row 1: 6 missing
      row 2: 5 missing
      row 3: 4 missing
      row 4: 3 missing
      row 5: 2 missing
      row 6: 1 missing
      row 7: 1 missing
    Rows 8-15: full.
    """
    pixels = full_pixels()
    cliff = [8, 6, 5, 4, 3, 2, 1, 1]
    for y, depth in enumerate(cliff):
        for x in range(depth):
            pixels.discard((x, y))
    return pixels

def corner_inner_pixels():
    """Inner corner at upper-left — small notch (3 pixels).

    row 0: 2 missing (x=0,1)
    row 1: 1 missing (x=0)
    rows 2-15: full.
    """
    pixels = full_pixels()
    pixels.discard((0, 0))
    pixels.discard((1, 0))
    pixels.discard((0, 1))
    return pixels

def draw_tile(img, tile_idx, pixels):
    ox = tile_idx * TILE_W
    for x, y in pixels:
        if 0 <= x < TILE_W and 0 <= y < TILE_H:
            img.putpixel((ox + x, y), (0, 0, 0, 255))

def main():
    img = Image.new("RGBA", (COLS * TILE_W, TILE_H), (0, 0, 0, 0))

    tiles = [
        ("FULL",         full_pixels()),
        ("SHORE_LL",     shore_ll_pixels()),
        ("SHORE_UL",     shore_ul_pixels()),
        ("CORNER_OUTER", corner_outer_pixels()),
        ("CORNER_INNER", corner_inner_pixels()),
    ]

    for idx, (name, pixels) in enumerate(tiles):
        draw_tile(img, idx, pixels)

    out = "assets/3_mountains.png"
    img.save(out)
    print(f"Saved {out}: {img.size[0]}×{img.size[1]} ({COLS} tiles)")

    for idx, (name, pixels) in enumerate(tiles):
        cnt = len(pixels)
        print(f"  Tile {idx} {name:15s}: {cnt:3d} px set ({256-cnt:2d} missing)")

if __name__ == "__main__":
    main()

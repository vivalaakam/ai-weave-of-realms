#!/usr/bin/env python3
"""Generate water tileset 2_water.png — 30 tiles (240×48), monochrome with alpha."""

from PIL import Image

TILE_W, TILE_H = 16, 16
COLS = 10
ROWS = 3

DIAMOND = {
    0: (6, 9), 1: (6, 9),
    2: (4, 11), 3: (3, 12),
    4: (2, 13), 5: (2, 13),
    6: (1, 14), 7: (1, 14),
    8: (1, 14), 9: (1, 14),
    10: (2, 13), 11: (2, 13),
    12: (3, 12), 13: (4, 11),
    14: (6, 9),
}

def diamond_pixels():
    return {(x, y) for y, (l, r) in DIAMOND.items() for x in range(l, r + 1)}

def shore_pixels(land_bit):
    """Single land side. land_bit: 1=W, 2=E, 8=N, 4=S."""
    profile = {
        0: (6, 9), 1: (6, 9), 2: (5, 10), 3: (4, 11),
        4: (4, 11), 5: (3, 12), 6: (3, 12), 7: (3, 12),
        8: (3, 12), 9: (3, 12), 10: (4, 11), 11: (4, 11),
        12: (5, 10), 13: (5, 10), 14: (6, 9),
    }
    pixels = set()
    for y in range(TILE_H):
        if y not in DIAMOND:
            continue
        d_l, d_r = DIAMOND[y]
        w_l, w_r = d_l, d_r
        if land_bit & 1:  # W land
            w_l = max(w_l, profile[y][0])
        if land_bit & 2:  # E land
            w_r = min(w_r, profile[y][1])
        if land_bit & 8:  # N land
            if y <= 3:
                continue
            if y <= 6:
                w_l, w_r = max(w_l, d_l + 1), min(w_r, d_r - 1)
        if land_bit & 4:  # S land
            if y >= 12:
                continue
            if y >= 10:
                w_l, w_r = max(w_l, d_l + 1), min(w_r, d_r - 1)
        if w_l <= w_r:
            for x in range(w_l, w_r + 1):
                pixels.add((x, y))
    return pixels

def corner_pixels(land_bits):
    """Two adjacent land sides."""
    profile = {
        0: (6, 9), 1: (6, 9), 2: (5, 10), 3: (5, 10),
        4: (4, 11), 5: (4, 11), 6: (4, 11), 7: (3, 12),
        8: (3, 12), 9: (4, 11), 10: (4, 11), 11: (5, 10),
        12: (5, 10), 13: (6, 9), 14: (6, 9),
    }
    pixels = set()
    for y in range(TILE_H):
        if y not in DIAMOND:
            continue
        d_l, d_r = DIAMOND[y]
        w_l, w_r = d_l, d_r
        if land_bits & 1:
            w_l = max(w_l, profile[y][0] + 1)
        if land_bits & 2:
            w_r = min(w_r, profile[y][1] - 1)
        if land_bits & 8:
            if y <= 5:
                continue
            w_l, w_r = max(w_l, d_l + 1), min(w_r, d_r - 1)
        if land_bits & 4:
            if y >= 10:
                continue
            w_l, w_r = max(w_l, d_l + 1), min(w_r, d_r - 1)
        if w_l <= w_r:
            for x in range(w_l, w_r + 1):
                pixels.add((x, y))
    return pixels

def peninsula_pixels(water_bit):
    """Water only on one side."""
    pixels = set()
    if water_bit == 1:  # W only
        for y in range(5, 12):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(l, l + 3):
                    if x <= r:
                        pixels.add((x, y))
    elif water_bit == 2:  # E only
        for y in range(5, 12):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(r - 2, r + 1):
                    if x >= l:
                        pixels.add((x, y))
    elif water_bit == 8:  # N only
        for y in range(0, 4):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(l, r + 1):
                    pixels.add((x, y))
        for y in range(4, 6):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(l + 2, r - 1):
                    if x <= r:
                        pixels.add((x, y))
    elif water_bit == 4:  # S only
        for y in range(11, 15):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(l, r + 1):
                    pixels.add((x, y))
        for y in range(9, 11):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(l + 2, r - 1):
                    if x <= r:
                        pixels.add((x, y))
    return pixels

def strait_pixels(land_bits):
    """Opposite land sides."""
    pixels = set()
    if land_bits == 0b1010:  # N+S land → H-channel
        for y in range(6, 10):
            if y in DIAMOND:
                l, r = DIAMOND[y]
                for x in range(l, r + 1):
                    pixels.add((x, y))
    elif land_bits == 0b0101:  # E+W land → V-channel
        for y in DIAMOND:
            mid = 8
            for x in range(mid - 1, mid + 2):
                if DIAMOND[y][0] <= x <= DIAMOND[y][1]:
                    pixels.add((x, y))
    return pixels

def pond_pixels():
    """Small isolated pond."""
    pixels = set()
    for y in range(5, 11):
        half = (y - 5) if y <= 7 else (10 - y)
        cx = 8
        for x in range(cx - half - 1, cx + half + 2):
            if 3 <= x <= 13:
                pixels.add((x, y))
    return pixels

def river_mouth_pixels(direction):
    """
    River entering from `direction` into water.
    direction: 'N','S','E','W'
    """
    pixels = set()
    base = diamond_pixels()
    # Draw a narrow channel from the water edge to the center
    channel = set()
    if direction == 'W':
        # Channel from left edge (x=0..2) to center at rows 7-8
        for y in range(6, 10):
            for x in range(0, 6):
                channel.add((x, y))
    elif direction == 'E':
        for y in range(6, 10):
            for x in range(10, 16):
                channel.add((x, y))
    elif direction == 'N':
        for y in range(0, 4):
            for x in range(6, 10):
                channel.add((x, y))
    elif direction == 'S':
        for y in range(12, 16):
            for x in range(6, 10):
                channel.add((x, y))
    return base | channel

def ripple(base, seed):
    """Remove ~8% interior pixels in random-looking pattern."""
    result = set(base)
    for x, y in list(result):
        if (x * 13 + y * 7 + seed * 31) % 12 == 0:
            result.discard((x, y))
    return result

def draw_tile(img, tile_idx, pixels):
    row, col = divmod(tile_idx, COLS)
    ox, oy = col * TILE_W, row * TILE_H
    for x, y in pixels:
        if 0 <= x < TILE_W and 0 <= y < TILE_H:
            img.putpixel((ox + x, oy + y), (0, 0, 0, 255))

def main():
    img = Image.new("RGBA", (COLS * TILE_W, ROWS * TILE_H), (0, 0, 0, 0))
    base = diamond_pixels()
    idx = 0

    # 0-3: Center variants
    draw_tile(img, idx, base); idx += 1
    draw_tile(img, idx, ripple(base, 1)); idx += 1
    draw_tile(img, idx, ripple(base, 3)); idx += 1
    draw_tile(img, idx, base); idx += 1

    # 4-7: Single-edge shores (W, E, N, S)
    draw_tile(img, idx, shore_pixels(0b0010)); idx += 1  # land E → water W,S,N
    draw_tile(img, idx, shore_pixels(0b0001)); idx += 1  # land W → water E,S,N
    draw_tile(img, idx, shore_pixels(0b0100)); idx += 1  # land S → water N,E,W
    draw_tile(img, idx, shore_pixels(0b1000)); idx += 1  # land N → water S,E,W

    # 8-11: Outer corners (NE, NW, SE, SW)
    draw_tile(img, idx, corner_pixels(0b1010)); idx += 1  # N+E → SW
    draw_tile(img, idx, corner_pixels(0b1001)); idx += 1  # N+W → SE
    draw_tile(img, idx, corner_pixels(0b0110)); idx += 1  # S+E → NW
    draw_tile(img, idx, corner_pixels(0b0101)); idx += 1  # S+W → NE

    # 12-15: Peninsulas (W, E, N, S)
    draw_tile(img, idx, peninsula_pixels(1)); idx += 1
    draw_tile(img, idx, peninsula_pixels(2)); idx += 1
    draw_tile(img, idx, peninsula_pixels(8)); idx += 1
    draw_tile(img, idx, peninsula_pixels(4)); idx += 1

    # 16-17: Straits
    draw_tile(img, idx, strait_pixels(0b1010)); idx += 1  # N+S land → H
    draw_tile(img, idx, strait_pixels(0b0101)); idx += 1  # E+W land → V

    # 18: Single-cell pond
    draw_tile(img, idx, pond_pixels()); idx += 1

    # 19: empty
    idx += 1

    # 20-27: River mouths (4 directions × 2 variants)
    for d in ('W', 'E', 'N', 'S'):
        draw_tile(img, idx, river_mouth_pixels(d)); idx += 1
        draw_tile(img, idx, ripple(river_mouth_pixels(d), 7)); idx += 1

    # 28-29: empty
    # (already at idx 28 after river mouths)

    out = "assets/2_water.png"
    img.save(out)
    print(f"Saved {out}: {img.size[0]}×{img.size[1]} ({COLS*ROWS} tiles)")

    for i in range(30):
        r, c = divmod(i, COLS)
        ox, oy = c * TILE_W, r * TILE_H
        cnt = sum(1 for y in range(TILE_H) for x in range(TILE_W)
                  if img.getpixel((ox + x, oy + y))[3] >= 128)
        print(f"  Tile {i:2d}: r{r} c{c}  {cnt:3d}px")

if __name__ == "__main__":
    main()

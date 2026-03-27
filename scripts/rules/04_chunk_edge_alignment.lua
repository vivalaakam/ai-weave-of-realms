--- Rule 04: Chunk edge alignment.
--
-- Chunk borders are connection interfaces for future neighbouring chunks.
-- The edge contract is:
--   - `road` and `river` may touch a chunk edge only at local positions
--     where `pos % 3 == 1` in 0-based indexing (1,4,7,...).
--   - `forest`, `mountain`, and `water` may also anchor only at those
--     positions; if the same kind appears on two neighbouring anchors
--     (distance 3), the border tiles between them must be the same kind.

local CHUNK_SIZE = 32
local NATURAL = { forest = true, mountain = true, water = true }
local CORRIDOR = { road = true, river = true }

local function is_anchor(pos)
    return pos % 3 == 1
end

local function edge_kind(map, chunk_x, chunk_y, side, pos)
    local gx
    local gy

    if side == 0 then
        gx = chunk_x + pos
        gy = chunk_y
    elseif side == 1 then
        gx = chunk_x + CHUNK_SIZE - 1
        gy = chunk_y + pos
    elseif side == 2 then
        gx = chunk_x + pos
        gy = chunk_y + CHUNK_SIZE - 1
    else
        gx = chunk_x
        gy = chunk_y + pos
    end

    return map.get(gx, gy)
end

local function valid_natural_fill(map, chunk_x, chunk_y, side, pos, kind)
    local left = pos - 1
    while left >= 0 and not is_anchor(left) do
        left = left - 1
    end

    local right = pos + 1
    while right < CHUNK_SIZE and not is_anchor(right) do
        right = right + 1
    end

    if left < 0 or right >= CHUNK_SIZE then
        return false
    end

    return edge_kind(map, chunk_x, chunk_y, side, left) == kind
        and edge_kind(map, chunk_x, chunk_y, side, right) == kind
end

return function(map)
    for chunk_y = 0, map.height - 1, CHUNK_SIZE do
        for chunk_x = 0, map.width - 1, CHUNK_SIZE do
            for side = 0, 3 do
                for pos = 0, CHUNK_SIZE - 1 do
                    local kind = edge_kind(map, chunk_x, chunk_y, side, pos)

                    if CORRIDOR[kind] and not is_anchor(pos) then
                        return false, string.format(
                            "chunk edge tile '%s' at chunk (%d,%d) side %d pos %d violates anchor rule",
                            kind, chunk_x / CHUNK_SIZE, chunk_y / CHUNK_SIZE, side, pos
                        )
                    end

                    if NATURAL[kind] and not is_anchor(pos)
                        and not valid_natural_fill(map, chunk_x, chunk_y, side, pos, kind)
                    then
                        return false, string.format(
                            "chunk edge tile '%s' at chunk (%d,%d) side %d pos %d must be part of an anchored continuous border segment",
                            kind, chunk_x / CHUNK_SIZE, chunk_y / CHUNK_SIZE, side, pos
                        )
                    end
                end
            end
        end
    end

    return true, nil
end

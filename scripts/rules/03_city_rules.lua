--- Rule 03: City placement rules.
--
-- For each city block on the map:
--   - A city occupies a 3×3 tile block at some (bx, by).
--   - The CityEntrance tile must be placed at (bx, by+2) — the bottom-left corner
--     of the 3×3 block, which is the south-facing tip of the isometric diamond.
--   - Only one city block per chunk (32×32 tiles).
--   - The 4 orthogonal neighbours of the CityEntrance that lie outside the 3×3 block
--     must be either "meadow" or "road" (no river, mountain, water, forest, resource, gold).
--
-- If no city tiles are present on the map, this rule passes trivially.

local CHUNK_SIZE = 32
local ALLOWED_NEAR_ENTRANCE = { meadow = true, road = true }

local function check_city(map, entrance_x, entrance_y)
    -- The CityEntrance is at (bx, by+2), so the 3×3 block top-left is (bx, by) = (entrance_x, entrance_y - 2)
    local bx = entrance_x
    local by = entrance_y - 2

    -- Neighbours of entrance outside the 3×3 block:
    -- Left  (bx-1, by+2), Right (bx+3, by+2) are outside the block horizontally.
    -- Below (bx,   by+3) is below the block.
    -- Above (bx,   by+1) is inside the block, so we skip it.
    -- Also check (bx-1, by+2) and (bx+3, by+2) for left/right of entrance row.
    local neighbours = {
        { bx - 1, by + 2 },  -- left of entrance
        { bx + 3, by + 2 },  -- right of entrance (3 wide block ends at bx+2)
        { bx,     by + 3 },  -- below entrance
        { bx + 1, by + 3 },  -- below middle of block bottom row
        { bx + 2, by + 3 },  -- below right of block bottom row
    }

    for _, nb in ipairs(neighbours) do
        local nx, ny = nb[1], nb[2]
        local kind = map.get(nx, ny)
        if kind ~= "out_of_bounds" and not ALLOWED_NEAR_ENTRANCE[kind] then
            return false, string.format(
                "tile '%s' at (%d,%d) adjacent to city entrance at (%d,%d) must be meadow or road",
                kind, nx, ny, entrance_x, entrance_y
            )
        end
    end

    return true, nil
end

return function(map)
    local cw = map.chunks_wide
    local ct = map.chunks_tall

    for cy = 0, ct - 1 do
        for cx = 0, cw - 1 do
            -- Scan this chunk for city_entrance tiles
            local chunk_ox = cx * CHUNK_SIZE
            local chunk_oy = cy * CHUNK_SIZE
            local entrance_count = 0
            local city_count     = 0

            for ly = 0, CHUNK_SIZE - 1 do
                for lx = 0, CHUNK_SIZE - 1 do
                    local gx   = chunk_ox + lx
                    local gy   = chunk_oy + ly
                    local kind = map.get(gx, gy)

                    if kind == "city" then
                        city_count = city_count + 1
                    elseif kind == "city_entrance" then
                        entrance_count = entrance_count + 1

                        local ok, reason = check_city(map, gx, gy)
                        if not ok then
                            return false, reason
                        end
                    end
                end
            end

            -- At most one entrance per chunk
            if entrance_count > 1 then
                return false, string.format(
                    "chunk (%d,%d) contains %d city entrances; at most 1 is allowed",
                    cx, cy, entrance_count
                )
            end

            -- If there's an entrance there must also be city tiles (and vice versa)
            if entrance_count == 1 and city_count == 0 then
                return false, string.format(
                    "chunk (%d,%d) has a city_entrance but no city tiles", cx, cy
                )
            end
            if city_count > 0 and entrance_count == 0 then
                return false, string.format(
                    "chunk (%d,%d) has city tiles but no city_entrance", cx, cy
                )
            end
        end
    end

    return true, nil
end

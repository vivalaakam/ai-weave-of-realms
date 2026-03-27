--- Rule 03: City placement rules.
--
-- For each city block on the map:
--   - A city occupies a 3×3 tile block at some (bx, by).
--   - The CityEntrance tile must be placed at (bx, by+2) — the bottom-left corner
--     of the 3×3 block, which is the south-facing tip of the isometric diamond.
--   - The 4 orthogonal neighbours of the CityEntrance that lie outside the 3×3 block
--     must be either "meadow" or "road" (no river, mountain, water, forest, resource, gold).
--
-- If no city tiles are present on the map, this rule passes trivially.

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
    local entrances = {}  -- list of {x, y}

    for gy = 0, map.height - 1 do
        for gx = 0, map.width - 1 do
            local kind = map.get(gx, gy)
            if kind == "city_entrance" then
                entrances[#entrances + 1] = { gx, gy }

                local ok, reason = check_city(map, gx, gy)
                if not ok then return false, reason end
            end
        end
    end

    return true, nil
end

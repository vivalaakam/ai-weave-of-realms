--- Map validator.
--
-- Checks that a generated map satisfies basic playability constraints.
-- Called after generation; if validation fails the map is rejected.
--
-- @param map  Table with the following fields:
--               map.chunks_wide  (number) — horizontal chunk count
--               map.chunks_tall  (number) — vertical chunk count
--               map.tiles        (table)  — flat array of TileKind strings,
--                                           row-major, total = chunks_wide*32 * chunks_tall*32
-- @return     boolean, string|nil — (true, nil) on success,
--                                   (false, "reason") on failure

local function validate(map)
    local total = #map.tiles
    if total == 0 then
        return false, "map has no tiles"
    end

    local counts = {
        grass    = 0,
        water    = 0,
        forest   = 0,
        mountain = 0,
        road     = 0,
        ruins    = 0,
    }

    for _, kind in ipairs(map.tiles) do
        if counts[kind] ~= nil then
            counts[kind] = counts[kind] + 1
        end
    end

    -- At least 50% of the map must be passable
    local passable = counts.grass + counts.forest + counts.road + counts.ruins
    if passable / total < 0.50 then
        return false, string.format(
            "too little passable terrain: %.1f%% (minimum 50%%)",
            passable / total * 100
        )
    end

    -- No more than 40% water
    if counts.water / total > 0.40 then
        return false, string.format(
            "too much water: %.1f%% (maximum 40%%)",
            counts.water / total * 100
        )
    end

    -- No more than 40% mountains
    if counts.mountain / total > 0.40 then
        return false, string.format(
            "too many mountains: %.1f%% (maximum 40%%)",
            counts.mountain / total * 100
        )
    end

    return true, nil
end

return validate

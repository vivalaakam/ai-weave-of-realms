--- Map validator.
--
-- Checks that a generated map satisfies basic playability constraints.
--
-- @param map  Table: map.chunks_wide, map.chunks_tall, map.tiles (flat array)
-- @return     boolean, string|nil — (true, nil) on success, (false, reason) on failure

local PASSABLE = {
    meadow = true, forest = true, road = true, bridge = true,
    city = true, city_entrance = true, village = true,
    merchant = true, ruins = true, gold = true, resource = true,
}

local function validate(map)
    local total = #map.tiles
    if total == 0 then
        return false, "map has no tiles"
    end

    local counts = {}
    for _, kind in ipairs(map.tiles) do
        counts[kind] = (counts[kind] or 0) + 1
    end

    -- At least 50% of the map must be passable
    local passable = 0
    for kind, n in pairs(counts) do
        if PASSABLE[kind] then passable = passable + n end
    end
    if passable / total < 0.50 then
        return false, string.format(
            "too little passable terrain: %.1f%% (minimum 50%%)",
            passable / total * 100
        )
    end

    -- No more than 40% combined impassable terrain
    local blocked = (counts["water"] or 0) + (counts["mountain"] or 0) + (counts["river"] or 0)
    if blocked / total > 0.40 then
        return false, string.format(
            "too much impassable terrain: %.1f%% (maximum 40%%)",
            blocked / total * 100
        )
    end

    return true, nil
end

return validate

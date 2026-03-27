--- Rule 01: Passable terrain ratio.
--
-- At least 50% of all tiles must be passable (not water, mountain, or river).

return function(map)
    local impassable = { water = true, mountain = true, river = true }
    local pass = 0

    for _, kind in ipairs(map.tiles) do
        if not impassable[kind] then
            pass = pass + 1
        end
    end

    local ratio = pass / #map.tiles
    if ratio < 0.50 then
        return false, string.format(
            "passable terrain %.1f%% is below the required 50%%", ratio * 100
        )
    end

    return true, nil
end

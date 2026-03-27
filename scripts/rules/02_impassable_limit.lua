--- Rule 02: Impassable terrain upper limit.
--
-- No more than 40% of all tiles may be impassable (water, mountain, or river).

return function(map)
    local impassable = { water = true, mountain = true, river = true }
    local blocked = 0

    for _, kind in ipairs(map.tiles) do
        if impassable[kind] then
            blocked = blocked + 1
        end
    end

    local ratio = blocked / #map.tiles
    if ratio > 0.40 then
        return false, string.format(
            "impassable terrain %.1f%% exceeds the 40%% limit", ratio * 100
        )
    end

    return true, nil
end

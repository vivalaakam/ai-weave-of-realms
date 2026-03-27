--- Map evaluator.
--
-- Computes a quality score for a fully generated map.
-- Higher scores indicate better maps.
--
-- @param map  Table: map.chunks_wide, map.chunks_tall, map.tiles (flat array)
-- @return     number — quality score (higher = better, 0.0 = worst)

local PASSABLE = {
    meadow = true, forest = true, road = true, bridge = true,
    city = true, city_entrance = true, village = true,
    merchant = true, ruins = true, gold = true, resource = true,
}

local function evaluate(map)
    local total = #map.tiles
    if total == 0 then return 0.0 end

    local counts = {}
    for _, kind in ipairs(map.tiles) do
        counts[kind] = (counts[kind] or 0) + 1
    end

    -- Reward passable terrain (>= 60% target)
    local passable = 0
    for kind, n in pairs(counts) do
        if PASSABLE[kind] then passable = passable + n end
    end
    local score = (passable / total) * 100.0

    -- Bonus for variety of points of interest
    local poi_kinds = {"ruins", "gold", "resource", "village", "merchant"}
    for _, poi in ipairs(poi_kinds) do
        if (counts[poi] or 0) > 0 then score = score + 5.0 end
    end

    -- Penalty for too many impassable tiles (> 30%)
    local blocked = (counts["water"] or 0) + (counts["mountain"] or 0) + (counts["river"] or 0)
    local blocked_ratio = blocked / total
    if blocked_ratio > 0.30 then
        score = score - (blocked_ratio - 0.30) * 50.0
    end

    return math.max(0.0, score)
end

return evaluate

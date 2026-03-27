--- Map evaluator.
--
-- Computes a quality score for a fully generated map.
-- Higher scores indicate better maps. The generator can use this score
-- to select the best map from multiple candidates.
--
-- @param map  Table with the following fields:
--               map.chunks_wide  (number) — horizontal chunk count
--               map.chunks_tall  (number) — vertical chunk count
--               map.tiles        (table)  — flat array of TileKind strings,
--                                           row-major, total = chunks_wide*32 * chunks_tall*32
-- @return     number — quality score (higher is better, 0.0 = worst)

local function evaluate(map)
    local total = #map.tiles
    if total == 0 then
        return 0.0
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

    -- Reward maps where passable terrain dominates (>= 60%)
    local passable = counts.grass + counts.forest + counts.road + counts.ruins
    local passable_ratio = passable / total
    local score = passable_ratio * 100.0

    -- Bonus for having at least one ruins tile (point of interest)
    if counts.ruins > 0 then
        score = score + 10.0
    end

    -- Penalty if water or mountain blocks more than 30% of the map
    local blocked_ratio = (counts.water + counts.mountain) / total
    if blocked_ratio > 0.30 then
        score = score - (blocked_ratio - 0.30) * 50.0
    end

    return math.max(0.0, score)
end

return evaluate

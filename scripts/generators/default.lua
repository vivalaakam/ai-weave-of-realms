--- Default chunk generator.
--
-- Called once per chunk during map generation.
-- Produces a 32×32 flat tile table using the provided SeededRng.
--
-- ## Pipeline support
-- When used as part of a multi-stage generator pipeline, this generator
-- receives an optional 4th argument `tiles` with the base tile table from
-- the previous pipeline stage.  If `tiles` is not nil, those tiles are used
-- as the starting point; otherwise the chunk is generated from scratch.
--
-- @param rng   SeededRng userdata — deterministic RNG seeded per chunk.
--              Methods: next_f64(), random_range_u32(lo, hi), random_bool(probability)
-- @param x     Chunk column index (0-based).
-- @param y     Chunk row index (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage (nil if first).
-- @return      table of 1024 strings, each a valid Tiles identifier.
--              Valid: "meadow","forest","mountain","water","road","river",
--                     "ruins","gold","resource","village","bridge","city",
--                     "city_entrance","merchant"

local CHUNK_SIZE = 32

local function generate_chunk(rng, x, y, tiles)
    -- If we have base tiles from a previous pipeline stage, use them as-is.
    -- Otherwise generate the terrain from scratch.
    if tiles ~= nil then
        return tiles
    end

    local result = {}

    for i = 1, CHUNK_SIZE * CHUNK_SIZE do
        local roll = rng:next_f64()

        if roll < 0.04 then
            result[i] = "water"
        elseif roll < 0.10 then
            result[i] = "mountain"
        elseif roll < 0.22 then
            result[i] = "forest"
        elseif roll < 0.27 then
            result[i] = "river"
        elseif roll < 0.30 then
            result[i] = "road"
        elseif roll < 0.32 then
            result[i] = "ruins"
        elseif roll < 0.33 then
            result[i] = "gold"
        elseif roll < 0.34 then
            result[i] = "resource"
        else
            result[i] = "meadow"
        end
    end

    return result
end

return generate_chunk

--- Default chunk generator.
--
-- Called once per chunk during map generation.
-- Produces a 32×32 flat tile table using the provided SeededRng.
--
-- @param rng  SeededRng userdata — deterministic RNG seeded per chunk.
--             Methods: next_f64(), random_range_u32(lo, hi), random_bool(probability)
-- @param x    Chunk column index (0-based).
-- @param y    Chunk row index (0-based).
-- @return     table of 1024 strings, each a valid Tiles identifier.
--             Valid: "meadow","forest","mountain","water","road","river",
--                    "ruins","gold","resource","village","bridge","city",
--                    "city_entrance","merchant"

local CHUNK_SIZE = 32

local function generate_chunk(rng, x, y)
    local tiles = {}

    for i = 1, CHUNK_SIZE * CHUNK_SIZE do
        local roll = rng:next_f64()

        if roll < 0.04 then
            tiles[i] = "water"
        elseif roll < 0.10 then
            tiles[i] = "mountain"
        elseif roll < 0.22 then
            tiles[i] = "forest"
        elseif roll < 0.27 then
            tiles[i] = "river"
        elseif roll < 0.30 then
            tiles[i] = "road"
        elseif roll < 0.32 then
            tiles[i] = "ruins"
        elseif roll < 0.33 then
            tiles[i] = "gold"
        elseif roll < 0.34 then
            tiles[i] = "resource"
        else
            tiles[i] = "meadow"
        end
    end

    return tiles
end

return generate_chunk

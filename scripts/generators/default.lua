--- Default chunk generator.
--
-- Called once per chunk during map generation.
-- Produces a 32×32 flat tile table using the provided SeededRng.
--
-- @param rng  SeededRng userdata — deterministic RNG seeded per chunk.
--             Methods: next_f64(), random_range_u32(lo, hi),
--                      random_bool(probability)
-- @param x    Chunk column index (0-based).
-- @param y    Chunk row index (0-based).
-- @return     table of 1024 strings, each a valid TileKind identifier.
--             Valid values: "grass", "water", "forest", "mountain", "road", "ruins"

local CHUNK_SIZE = 32

local function generate_chunk(rng, x, y)
    local tiles = {}

    for i = 1, CHUNK_SIZE * CHUNK_SIZE do
        local roll = rng:next_f64()

        if roll < 0.05 then
            tiles[i] = "water"
        elseif roll < 0.15 then
            tiles[i] = "mountain"
        elseif roll < 0.30 then
            tiles[i] = "forest"
        elseif roll < 0.33 then
            tiles[i] = "road"
        elseif roll < 0.34 then
            tiles[i] = "ruins"
        else
            tiles[i] = "grass"
        end
    end

    return tiles
end

return generate_chunk

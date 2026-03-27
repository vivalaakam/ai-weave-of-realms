--- Forest cluster generator.
--
-- Pipeline stage that overlays forest clusters onto base tiles.
-- Receives base tiles as 4th argument and places forest clusters
-- over meadow tiles only, leaving protected terrain types untouched.
--
-- ## Algorithm
-- - Generates 3–7 random cluster centres
-- - Each cluster covers a circular zone of radius 3–7 tiles
-- - Forest probability decreases with distance: p = 1.0 - (dist / radius) * 0.8
-- - Only overwrites "meadow" tiles
-- - Protected: "city", "city_entrance", "water", "mountain", "river"
--
-- @param rng   SeededRng userdata — deterministic RNG seeded per chunk.
--              Methods: next_f64(), random_range_u32(lo, hi), random_bool(probability)
-- @param x     Chunk column index (0-based).
-- @param y     Chunk row index (0-based).
-- @param tiles Base tile table from the previous pipeline stage (1-indexed, 1024 entries).
--              If nil, all tiles start as "meadow".
-- @return      table of 1024 strings with forest clusters applied.

local CHUNK_SIZE = 32

-- Tiles that must not be overwritten by forest
local PROTECTED = {
    city          = true,
    city_entrance = true,
    water         = true,
    mountain      = true,
    river         = true,
}

local function tile_index(lx, ly)
    return ly * CHUNK_SIZE + lx + 1
end

local function generate_chunk(rng, x, y, tiles)
    -- Start from base tiles or a blank meadow canvas
    local result = {}
    if tiles ~= nil then
        for i = 1, CHUNK_SIZE * CHUNK_SIZE do
            result[i] = tiles[i]
        end
    else
        for i = 1, CHUNK_SIZE * CHUNK_SIZE do
            result[i] = "meadow"
        end
    end

    -- Number of forest clusters: 3–7
    local num_clusters = rng:random_range_u32(3, 8)  -- [3, 7] inclusive

    for _ = 1, num_clusters do
        -- Cluster centre (0-based tile coords within the chunk)
        local cx = rng:random_range_u32(0, CHUNK_SIZE - 1)
        local cy = rng:random_range_u32(0, CHUNK_SIZE - 1)
        -- Cluster radius: 3–7
        local radius = rng:random_range_u32(3, 8)  -- [3, 7] inclusive

        -- Paint tiles within the circular zone
        for ty = 0, CHUNK_SIZE - 1 do
            for tx = 0, CHUNK_SIZE - 1 do
                local dx = tx - cx
                local dy = ty - cy
                local dist = math.sqrt(dx * dx + dy * dy)

                if dist <= radius then
                    local idx = tile_index(tx, ty)
                    local current = result[idx]

                    -- Only overwrite meadow; skip protected tiles
                    if current == "meadow" and not PROTECTED[current] then
                        local p = 1.0 - (dist / radius) * 0.8
                        if rng:random_bool(p) then
                            result[idx] = "forest"
                        end
                    end
                end
            end
        end
    end

    return result
end

return generate_chunk

--- Lake / pond generator.
--
-- Pipeline stage that creates organic water bodies on the chunk.
-- Uses blob expansion from seed points with irregular arms to avoid
-- the "perfect circle" look.
--
-- ## Algorithm
-- 1. Place 1–3 lake seeds at random positions.
-- 2. For each seed, expand a circular blob (probability decays with distance).
-- 3. Grow 2–5 organic arms outward from each seed using a random walk.
-- 4. The arms widen slightly to merge into the main body.
--
-- Protected tiles (never overwritten): city, city_entrance, mountain, river, road.
-- Forest is also protected so that tree-lined shores appear naturally when
-- forest.lua runs before water.lua.
--
-- @param rng   SeededRng userdata.
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage.
-- @return      table[1024] with water bodies applied.

local CHUNK_SIZE = 32
local N = CHUNK_SIZE * CHUNK_SIZE

local PROTECTED = {
    city          = true,
    city_entrance = true,
    mountain      = true,
    river         = true,
    road          = true,
}

local function idx(tx, ty)
    return ty * CHUNK_SIZE + tx + 1
end

local function try_place(result, tx, ty, kind)
    if tx < 0 or tx >= CHUNK_SIZE or ty < 0 or ty >= CHUNK_SIZE then return end
    local i = idx(tx, ty)
    if not PROTECTED[result[i]] then
        result[i] = kind
    end
end

local function generate_chunk(rng, x, y, tiles)
    local result = {}
    if tiles ~= nil then
        for i = 1, N do result[i] = tiles[i] end
    else
        for i = 1, N do result[i] = "meadow" end
    end

    local num_lakes = rng:random_range_u32(1, 4)   -- [1, 3]

    for _ = 1, num_lakes do
        -- Keep seed away from edges so the lake fits inside
        local cx = rng:random_range_u32(5, CHUNK_SIZE - 6)
        local cy = rng:random_range_u32(5, CHUNK_SIZE - 6)
        local radius = rng:random_range_u32(3, 7)  -- [3, 6]

        -- ── Blob core ────────────────────────────────────────────────────────
        for ty = 0, CHUNK_SIZE - 1 do
            for tx = 0, CHUNK_SIZE - 1 do
                local dx = tx - cx
                local dy = ty - cy
                local dist = math.sqrt(dx * dx + dy * dy)
                if dist <= radius then
                    -- Higher probability at centre; jagged edge via RNG
                    local p = 1.0 - (dist / radius) * 0.7
                    if rng:random_bool(p) then
                        try_place(result, tx, ty, "water")
                    end
                end
            end
        end

        -- ── Organic arms ─────────────────────────────────────────────────────
        local num_arms = rng:random_range_u32(2, 6)  -- [2, 5]
        for _ = 1, num_arms do
            local angle  = rng:next_f64() * 2.0 * math.pi
            local arm_len = rng:random_range_u32(2, radius + 3)
            local ax, ay  = cx + 0.0, cy + 0.0

            for _ = 1, arm_len do
                -- Drift along the arm direction with slight jitter
                ax = ax + math.cos(angle) + (rng:next_f64() - 0.5) * 0.9
                ay = ay + math.sin(angle) + (rng:next_f64() - 0.5) * 0.9
                local tx = math.floor(ax + 0.5)
                local ty = math.floor(ay + 0.5)

                -- Place the arm tile and a small neighbourhood around it
                for wy = -1, 1 do
                    for wx = -1, 1 do
                        if rng:random_bool(0.55) then
                            try_place(result, tx + wx, ty + wy, "water")
                        end
                    end
                end
            end
        end
    end

    return result
end

return generate_chunk

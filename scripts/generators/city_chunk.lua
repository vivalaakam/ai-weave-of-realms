--- City chunk generator.
--
-- Generates a 32×32 chunk that contains exactly one city (3×3 block).
-- Layout:
--   - City block placed at a random position (bx, by) such that the full 3×3 block
--     fits within the chunk: bx ∈ [1, 28], by ∈ [1, 26].
--   - City tiles occupy (bx..bx+2, by..by+1) — 6 "city" tiles.
--   - CityEntrance tile placed at (bx, by+2) — south tip of the isometric diamond.
--   - The 4 orthogonal neighbours of the entrance outside the 3×3 block are forced
--     to "meadow" or "road" (alternating by RNG).
--   - Remaining tiles are wilderness (same probabilities as default generator).
--
-- ## Pipeline support
-- When used as part of a multi-stage generator pipeline, this generator
-- accepts an optional 4th argument `tiles` with the base tile table from
-- the previous pipeline stage.  If `tiles` is not nil, those tiles are used
-- as the wilderness base instead of generating it from scratch.
--
-- @param rng   SeededRng userdata.
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage (nil if first).
-- @return      table[1024] of tile name strings.

local CHUNK_SIZE = 32

local function tile_index(lx, ly)
    return ly * CHUNK_SIZE + lx + 1
end

local function generate_chunk(rng, x, y, tiles)
    local result = {}

    -- ── Step 1: fill with wilderness (or use base tiles) ─────────────────────
    if tiles ~= nil then
        -- Use tiles from the previous pipeline stage as the base
        for i = 1, CHUNK_SIZE * CHUNK_SIZE do
            result[i] = tiles[i]
        end
    else
        -- Generate wilderness from scratch
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
    end

    -- ── Step 2: place city block ─────────────────────────────────────────────
    -- bx ∈ [1, 28] so that block columns bx..bx+2 are within [0..31]
    -- by ∈ [1, 26] so that block rows  by..by+2 are within [0..31]
    -- (margin of 1 ensures we can also place entrance neighbours inside chunk)
    local bx = rng:random_range_u32(1, 28)  -- inclusive [1, 28]
    local by = rng:random_range_u32(1, 26)  -- inclusive [1, 26]

    -- 3×2 city tiles (top two rows of the 3×3 block)
    for dy = 0, 1 do
        for dx = 0, 2 do
            result[tile_index(bx + dx, by + dy)] = "city"
        end
    end

    -- CityEntrance at bottom-left (bx, by+2) — south isometric tip
    result[tile_index(bx, by + 2)] = "city_entrance"
    -- Fill the rest of the bottom row as city (right two cells)
    result[tile_index(bx + 1, by + 2)] = "city"
    result[tile_index(bx + 2, by + 2)] = "city"

    -- ── Step 3: clear tiles adjacent to the entrance ─────────────────────────
    -- Neighbours outside the 3×3 block must be meadow or road only.
    local entrance_neighbours = {
        { bx - 1, by + 2 },
        { bx + 3, by + 2 },
        { bx,     by + 3 },
        { bx + 1, by + 3 },
        { bx + 2, by + 3 },
    }

    for _, nb in ipairs(entrance_neighbours) do
        local nx, ny = nb[1], nb[2]
        if nx >= 0 and nx < CHUNK_SIZE and ny >= 0 and ny < CHUNK_SIZE then
            if rng:random_bool(0.7) then
                result[tile_index(nx, ny)] = "meadow"
            else
                result[tile_index(nx, ny)] = "road"
            end
        end
    end

    return result
end

return generate_chunk

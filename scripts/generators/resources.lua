--- Resource deposit generator.
--
-- Pipeline stage that places resource deposits and a gold mine on the chunk.
--
-- ## Placement rules
-- - Exactly 1 gold mine ("gold") per chunk.
-- - 4–7 resource deposits ("resource") per chunk.
-- - Minimum spacing of MIN_SPACING tiles between any two placed items
--   (gold counts toward the spacing constraint).
-- - Items cannot be placed within SAFE_RADIUS tiles of a city, city_entrance,
--   or village tile (prevents resources from touching settlements).
-- - Items are only placed on "meadow" tiles.
--
-- If a placement attempt fails after MAX_TRIES random draws, the item is
-- silently skipped (so a heavily-built chunk may have fewer than 4 resources).
--
-- @param rng   SeededRng userdata.
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage.
-- @return      table[1024] with resources applied.

local CHUNK_SIZE  = 32
local N           = CHUNK_SIZE * CHUNK_SIZE
local MIN_SPACING = 4    -- minimum tile distance between any two resource/gold tiles
local SAFE_RADIUS = 2    -- minimum tile distance from city / village tiles
local MAX_TRIES   = 60   -- rejection-sampling attempts per item

-- Tiles that a resource may be placed on
local PLACEABLE = { meadow = true }

-- Tiles that must not be within SAFE_RADIUS of a resource
local SETTLEMENT = { city = true, city_entrance = true, village = true }

local function tile_idx(tx, ty)
    return ty * CHUNK_SIZE + tx + 1
end

local function dist(x1, y1, x2, y2)
    local dx = x1 - x2
    local dy = y1 - y2
    return math.sqrt(dx * dx + dy * dy)
end

--- Returns true when (tx, ty) is within SAFE_RADIUS of any settlement tile.
local function near_settlement(result, tx, ty)
    for dy = -SAFE_RADIUS, SAFE_RADIUS do
        for dx = -SAFE_RADIUS, SAFE_RADIUS do
            local nx = tx + dx
            local ny = ty + dy
            if nx >= 0 and nx < CHUNK_SIZE and ny >= 0 and ny < CHUNK_SIZE then
                if SETTLEMENT[result[tile_idx(nx, ny)]] then
                    return true
                end
            end
        end
    end
    return false
end

--- Returns true when (tx, ty) is at least MIN_SPACING tiles from every placed item.
local function spaced_ok(placed, tx, ty)
    for _, p in ipairs(placed) do
        if dist(tx, ty, p[1], p[2]) < MIN_SPACING then
            return false
        end
    end
    return true
end

--- Attempt to place `kind` at a valid random position.
-- Returns true on success.
local function try_place(result, rng, placed, kind)
    for _ = 1, MAX_TRIES do
        -- Keep 1 tile away from edges for a cleaner look
        local tx = rng:random_range_u32(1, CHUNK_SIZE - 2)
        local ty = rng:random_range_u32(1, CHUNK_SIZE - 2)
        local i  = tile_idx(tx, ty)

        if PLACEABLE[result[i]]
            and not near_settlement(result, tx, ty)
            and spaced_ok(placed, tx, ty)
        then
            result[i]       = kind
            placed[#placed + 1] = { tx, ty }
            return true
        end
    end
    return false
end

local function generate_chunk(rng, x, y, tiles)
    local result = {}
    if tiles ~= nil then
        for i = 1, N do result[i] = tiles[i] end
    else
        for i = 1, N do result[i] = "meadow" end
    end

    local placed = {}

    -- Gold mine first so it gets the best spacing slot
    try_place(result, rng, placed, "gold")

    -- Resource deposits: 4–7
    local num_resources = rng:random_range_u32(4, 8)  -- inclusive [4, 7]
    for _ = 1, num_resources do
        try_place(result, rng, placed, "resource")
    end

    return result
end

return generate_chunk

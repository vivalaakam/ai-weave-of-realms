--- Road generator.
--
-- Pipeline stage that traces 0–2 roads across the chunk.
-- Roads connect two different edges of the chunk with a mostly-straight
-- path that navigates around impassable terrain.
--
-- ## Algorithm
-- 1. 30 % chance: no road; 45 % chance: 1 road; 25 % chance: 2 roads.
-- 2. Roads prefer to connect opposite edges (forming through-routes).
-- 3. The path uses an 85 %-direct / 15 %-meander walk.
-- 4. When blocked by water, mountain, or river, the walker tries the
--    perpendicular direction and then the reverse to find a way around.
-- 5. Entry/exit points are kept near the centre third of each edge
--    so the road does not hug the corner.
--
-- Protected (never overwritten): city, city_entrance, water, mountain, river.
--
-- @param rng   SeededRng userdata.
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage.
-- @return      table[1024] with roads applied.

local CHUNK_SIZE = 32
local N = CHUNK_SIZE * CHUNK_SIZE

local BLOCKED = {
    city          = true,
    city_entrance = true,
    water         = true,
    mountain      = true,
    river         = true,
}

local function idx(tx, ty)
    return ty * CHUNK_SIZE + tx + 1
end

--- Returns a random entry/exit point on `edge` near the centre third.
local function edge_point(rng, edge)
    local lo = math.floor(CHUNK_SIZE / 3)
    local hi = math.floor(2 * CHUNK_SIZE / 3)
    local pos = rng:random_range_u32(lo, hi)
    if edge == 0 then return pos, 0
    elseif edge == 1 then return CHUNK_SIZE - 1, pos
    elseif edge == 2 then return pos, CHUNK_SIZE - 1
    else             return 0, pos
    end
end

--- Try to place a road tile at (tx, ty) if not blocked.
local function place(result, tx, ty)
    if tx < 0 or tx >= CHUNK_SIZE or ty < 0 or ty >= CHUNK_SIZE then return false end
    local i = idx(tx, ty)
    if not BLOCKED[result[i]] then
        result[i] = "road"
        return true
    end
    return false
end

--- Compute the next step toward (ex, ey) from (cx, cy).
-- Returns up to three candidate (mx, my) moves in priority order.
local function candidate_moves(cx, cy, ex, ey, rng)
    local dx = ex - cx
    local dy = ey - cy
    local mx, my

    if rng:random_bool(0.85) then
        -- Primary: step toward exit along the dominant axis
        if math.abs(dx) >= math.abs(dy) then
            mx, my = (dx > 0 and 1 or -1), 0
        else
            mx, my = 0, (dy > 0 and 1 or -1)
        end
    else
        -- Meander: step along minor axis
        if math.abs(dx) >= math.abs(dy) then
            mx, my = 0, (dy ~= 0 and (dy > 0 and 1 or -1) or (rng:random_bool(0.5) and 1 or -1))
        else
            mx, my = (dx ~= 0 and (dx > 0 and 1 or -1) or (rng:random_bool(0.5) and 1 or -1)), 0
        end
    end

    -- Fallback moves when blocked: perpendicular then reverse direction
    local alt1_x, alt1_y = my, mx                          -- 90° clockwise
    local alt2_x, alt2_y = -my, -mx                        -- 90° counter-clockwise
    local alt3_x, alt3_y = -mx, -my                        -- backtrack

    return { {mx, my}, {alt1_x, alt1_y}, {alt2_x, alt2_y}, {alt3_x, alt3_y} }
end

local function generate_chunk(rng, x, y, tiles)
    local result = {}
    if tiles ~= nil then
        for i = 1, N do result[i] = tiles[i] end
    else
        for i = 1, N do result[i] = "meadow" end
    end

    -- Decide how many roads
    local roll = rng:next_f64()
    local num_roads
    if roll < 0.30 then
        num_roads = 0
    elseif roll < 0.75 then
        num_roads = 1
    else
        num_roads = 2
    end

    for _ = 1, num_roads do
        local start_edge = rng:random_range_u32(0, 3)
        -- Prefer opposite edge (through-road) with 70 % probability
        local end_edge
        if rng:random_bool(0.7) then
            end_edge = (start_edge + 2) % 4
        else
            end_edge = (start_edge + rng:random_range_u32(1, 3)) % 4
        end

        local sx, sy = edge_point(rng, start_edge)
        local ex, ey = edge_point(rng, end_edge)

        local cx, cy = sx, sy
        local max_steps = CHUNK_SIZE * 4
        local last_mx, last_my = 0, 0

        for _ = 1, max_steps do
            place(result, cx, cy)

            -- Check exit condition
            local done = false
            if end_edge == 0 and cy <= 0 then done = true end
            if end_edge == 2 and cy >= CHUNK_SIZE - 1 then done = true end
            if end_edge == 1 and cx >= CHUNK_SIZE - 1 then done = true end
            if end_edge == 3 and cx <= 0 then done = true end
            if done then break end

            local dx = ex - cx
            local dy = ey - cy
            if math.abs(dx) + math.abs(dy) == 0 then break end

            -- Try moves in priority order until one is unblocked
            local moves = candidate_moves(cx, cy, ex, ey, rng)
            local moved = false
            for _, mv in ipairs(moves) do
                local nx = math.max(0, math.min(CHUNK_SIZE - 1, cx + mv[1]))
                local ny = math.max(0, math.min(CHUNK_SIZE - 1, cy + mv[2]))
                if not BLOCKED[result[idx(nx, ny)]] then
                    last_mx, last_my = mv[1], mv[2]
                    cx, cy = nx, ny
                    moved = true
                    break
                end
            end

            -- If every direction is blocked, force the primary step
            if not moved then
                cx = math.max(0, math.min(CHUNK_SIZE - 1, cx + moves[1][1]))
                cy = math.max(0, math.min(CHUNK_SIZE - 1, cy + moves[1][2]))
            end
        end
    end

    return result
end

return generate_chunk

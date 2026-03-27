--- River generator.
--
-- Pipeline stage that carves 1–2 rivers across the chunk.
-- Each river enters from one edge and exits from a different edge,
-- creating continuous through-routes that connect across chunk boundaries.
--
-- ## Algorithm
-- 1. Pick a start edge and an exit edge (prefer opposite sides for long rivers).
-- 2. Pick a random entry/exit point on those edges.
-- 3. Walk from start to exit using a biased drunkard's walk:
--    - 65 % chance: step toward the exit.
--    - 35 % chance: perpendicular meander.
-- 4. Occasionally widen the river to 2 tiles for realism.
-- 5. At mountain tiles the river detours around them.
--
-- Protected (never overwritten): city, city_entrance, mountain, water, road.
--
-- @param rng   SeededRng userdata.
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage.
-- @return      table[1024] with rivers applied.

local CHUNK_SIZE = 32
local N = CHUNK_SIZE * CHUNK_SIZE

local PROTECTED = {
    city          = true,
    city_entrance = true,
    mountain      = true,
    water         = true,
    road          = true,
}

local function idx(tx, ty)
    return ty * CHUNK_SIZE + tx + 1
end

--- Returns (px, py) for a random point on `edge` (0=top,1=right,2=bottom,3=left).
-- Keeps a margin of 3 from corners for cleaner entry/exit.
local function edge_point(rng, edge)
    local pos = rng:random_range_u32(3, CHUNK_SIZE - 4)
    if edge == 0 then return pos, 0
    elseif edge == 1 then return CHUNK_SIZE - 1, pos
    elseif edge == 2 then return pos, CHUNK_SIZE - 1
    else             return 0, pos
    end
end

--- Place a river tile at (tx, ty) if not protected.
local function place(result, tx, ty)
    if tx < 0 or tx >= CHUNK_SIZE or ty < 0 or ty >= CHUNK_SIZE then return end
    local i = idx(tx, ty)
    if not PROTECTED[result[i]] then
        result[i] = "river"
    end
end

--- One step of the biased drunkard's walk toward (ex, ey).
-- Returns (new_cx, new_cy, blocked) where blocked = true when the chosen
-- cell is a mountain and we should try an alternative step.
local function walk_step(result, cx, cy, ex, ey, rng)
    local dx = ex - cx
    local dy = ey - cy

    local mx, my
    if rng:random_bool(0.65) then
        -- Move toward exit
        if math.abs(dx) >= math.abs(dy) then
            mx, my = (dx > 0 and 1 or -1), 0
        else
            mx, my = 0, (dy > 0 and 1 or -1)
        end
    else
        -- Perpendicular meander
        if math.abs(dx) >= math.abs(dy) then
            mx, my = 0, (rng:random_bool(0.5) and 1 or -1)
        else
            mx, my = (rng:random_bool(0.5) and 1 or -1), 0
        end
    end

    local nx = math.max(0, math.min(CHUNK_SIZE - 1, cx + mx))
    local ny = math.max(0, math.min(CHUNK_SIZE - 1, cy + my))

    -- If blocked by mountain, try the alternate axis
    if result[idx(nx, ny)] == "mountain" then
        if mx ~= 0 then
            mx, my = 0, (dy > 0 and 1 or -1)
        else
            mx, my = (dx > 0 and 1 or -1), 0
        end
        nx = math.max(0, math.min(CHUNK_SIZE - 1, cx + mx))
        ny = math.max(0, math.min(CHUNK_SIZE - 1, cy + my))
    end

    return nx, ny
end

local function generate_chunk(rng, x, y, tiles)
    local result = {}
    if tiles ~= nil then
        for i = 1, N do result[i] = tiles[i] end
    else
        for i = 1, N do result[i] = "meadow" end
    end

    local num_rivers = rng:random_range_u32(1, 3)  -- [1, 2]

    for _ = 1, num_rivers do
        local start_edge = rng:random_range_u32(0, 3)
        -- 70 % chance: opposite edge (longer river); 30 % chance: adjacent
        local end_edge
        if rng:random_bool(0.7) then
            end_edge = (start_edge + 2) % 4
        else
            end_edge = (start_edge + rng:random_range_u32(1, 3)) % 4
        end

        local sx, sy = edge_point(rng, start_edge)
        local ex, ey = edge_point(rng, end_edge)

        local cx, cy = sx, sy
        local max_steps = CHUNK_SIZE * 5

        for _ = 1, max_steps do
            place(result, cx, cy)

            -- Occasionally widen the river by one perpendicular tile
            if rng:random_bool(0.25) then
                local dx = ex - cx
                local dy = ey - cy
                local len = math.sqrt(dx * dx + dy * dy)
                if len > 0.5 then
                    -- Perpendicular unit vector, rounded
                    local px = math.floor(-dy / len + 0.5)
                    local py = math.floor(dx / len + 0.5)
                    place(result, cx + px, cy + py)
                end
            end

            -- Stop when we reach the exit edge
            local done = false
            if end_edge == 0 and cy <= 0 then done = true end
            if end_edge == 2 and cy >= CHUNK_SIZE - 1 then done = true end
            if end_edge == 1 and cx >= CHUNK_SIZE - 1 then done = true end
            if end_edge == 3 and cx <= 0 then done = true end
            if done then break end

            local dx = ex - cx
            local dy = ey - cy
            if math.abs(dx) + math.abs(dy) == 0 then break end

            cx, cy = walk_step(result, cx, cy, ex, ey, rng)
        end
    end

    return result
end

return generate_chunk

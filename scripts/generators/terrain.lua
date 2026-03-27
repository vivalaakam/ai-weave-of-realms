--- Full terrain generator (combined pipeline).
--
-- Single-script equivalent of the full generator pipeline:
--   water → river → forest → mountain → road → city → resources
--
-- Each stage runs in sequence on the same tile table, sharing the one
-- chunk-level RNG.  The result is fully deterministic: same seed + same
-- chunk coordinates always produce the same chunk.
--
-- ## Stage order
-- 1. Base canvas    — fill everything with "meadow".
-- 2. Water          — 1–3 organic lakes with arms.
-- 3. Rivers         — 1–2 edge-to-edge rivers with meandering.
-- 4. Forest         — 3–7 circular forest clusters.
-- 5. Mountains      — 1–3 ridge polylines.
-- 6. Roads          — 0–2 mostly-straight through-roads.
-- 7. City           — one 3×3 city block with entrance per chunk.
-- 8. Resources      — 1 gold mine + 4–7 resource deposits.
--
-- ## Pipeline support
-- Accepts an optional 4th argument `tiles`.  When present, the base-canvas
-- step is skipped and those tiles are used as the starting point instead,
-- letting this generator be composed with others.
--
-- @param rng   SeededRng userdata (methods: next_f64, random_range_u32, random_bool).
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table (nil = generate from scratch).
-- @return      table[1024] of tile name strings.

local CHUNK_SIZE = 32
local N          = CHUNK_SIZE * CHUNK_SIZE

-- ─── Shared helpers ───────────────────────────────────────────────────────────

local function idx(tx, ty)
    return ty * CHUNK_SIZE + tx + 1
end

local function clamp(v, lo, hi)
    return math.max(lo, math.min(hi, v))
end

local function is_edge_anchor(pos)
    return pos % 3 == 1
end

local function random_edge_anchor(rng, lo, hi)
    local choices = {}
    for pos = lo, hi do
        if is_edge_anchor(pos) then
            choices[#choices + 1] = pos
        end
    end
    return choices[rng:random_range_u32(1, #choices + 1)]
end

local function dist2d(x1, y1, x2, y2)
    local dx = x1 - x2
    local dy = y1 - y2
    return math.sqrt(dx * dx + dy * dy)
end

--- Place `kind` at (tx, ty) unless it is in the `protected` set.
local function place(result, tx, ty, kind, protected)
    if tx < 0 or tx >= CHUNK_SIZE or ty < 0 or ty >= CHUNK_SIZE then return end
    local i = idx(tx, ty)
    if not protected[result[i]] then
        result[i] = kind
    end
end

-- ─── Stage 1 – base canvas ────────────────────────────────────────────────────

local function stage_base(result, tiles)
    if tiles ~= nil then
        for i = 1, N do result[i] = tiles[i] end
    else
        for i = 1, N do result[i] = "meadow" end
    end
end

-- ─── Stage 2 – water bodies ───────────────────────────────────────────────────

local WATER_PROTECTED = {
    city = true, city_entrance = true,
    mountain = true, river = true, road = true,
}

local function stage_water(result, rng)
    local num_lakes = rng:random_range_u32(1, 4)  -- [1, 3]

    for _ = 1, num_lakes do
        local cx     = rng:random_range_u32(5, CHUNK_SIZE - 6)
        local cy     = rng:random_range_u32(5, CHUNK_SIZE - 6)
        local radius = rng:random_range_u32(3, 7)

        -- Circular blob core
        for ty = 0, CHUNK_SIZE - 1 do
            for tx = 0, CHUNK_SIZE - 1 do
                local d = dist2d(tx, ty, cx, cy)
                if d <= radius then
                    if rng:random_bool(1.0 - (d / radius) * 0.7) then
                        place(result, tx, ty, "water", WATER_PROTECTED)
                    end
                end
            end
        end

        -- Organic arms
        local num_arms = rng:random_range_u32(2, 6)
        for _ = 1, num_arms do
            local angle   = rng:next_f64() * 2.0 * math.pi
            local arm_len = rng:random_range_u32(2, radius + 3)
            local ax, ay  = cx + 0.0, cy + 0.0

            for _ = 1, arm_len do
                ax = ax + math.cos(angle) + (rng:next_f64() - 0.5) * 0.9
                ay = ay + math.sin(angle) + (rng:next_f64() - 0.5) * 0.9
                local tx = math.floor(ax + 0.5)
                local ty = math.floor(ay + 0.5)
                for wy = -1, 1 do
                    for wx = -1, 1 do
                        if rng:random_bool(0.55) then
                            place(result, tx + wx, ty + wy, "water", WATER_PROTECTED)
                        end
                    end
                end
            end
        end
    end
end

-- ─── Stage 3 – rivers ─────────────────────────────────────────────────────────

local RIVER_PROTECTED = {
    city = true, city_entrance = true,
    mountain = true, water = true, road = true,
}

--- Random entry/exit point on `edge` (0=top, 1=right, 2=bottom, 3=left).
local function edge_point(rng, edge)
    local pos = random_edge_anchor(rng, 3, CHUNK_SIZE - 4)
    if edge == 0 then return pos, 0
    elseif edge == 1 then return CHUNK_SIZE - 1, pos
    elseif edge == 2 then return pos, CHUNK_SIZE - 1
    else             return 0, pos
    end
end

local function stage_rivers(result, rng)
    local num_rivers = rng:random_range_u32(1, 3)

    for _ = 1, num_rivers do
        local se = rng:random_range_u32(0, 3)
        local ee = rng:random_bool(0.7) and ((se + 2) % 4)
                   or ((se + rng:random_range_u32(1, 3)) % 4)

        local cx, cy = edge_point(rng, se)
        local ex, ey = edge_point(rng, ee)

        for _ = 1, CHUNK_SIZE * 5 do
            place(result, cx, cy, "river", RIVER_PROTECTED)

            -- Occasional width-2 broadening
            if rng:random_bool(0.25) then
                local dx  = ex - cx
                local dy  = ey - cy
                local len = math.sqrt(dx * dx + dy * dy)
                if len > 0.5 then
                    local px = math.floor(-dy / len + 0.5)
                    local py = math.floor( dx / len + 0.5)
                    place(result, cx + px, cy + py, "river", RIVER_PROTECTED)
                end
            end

            -- Exit check
            if (ee == 0 and cy <= 0) or (ee == 2 and cy >= CHUNK_SIZE - 1)
            or (ee == 1 and cx >= CHUNK_SIZE - 1) or (ee == 3 and cx <= 0) then
                break
            end

            local dx = ex - cx
            local dy = ey - cy
            if math.abs(dx) + math.abs(dy) == 0 then break end

            -- Step: 65 % toward exit, 35 % meander
            local mx, my
            if rng:random_bool(0.65) then
                if math.abs(dx) >= math.abs(dy) then mx, my = (dx > 0 and 1 or -1), 0
                else                                  mx, my = 0, (dy > 0 and 1 or -1) end
            else
                if math.abs(dx) >= math.abs(dy) then mx, my = 0, (rng:random_bool(0.5) and 1 or -1)
                else                                  mx, my = (rng:random_bool(0.5) and 1 or -1), 0 end
            end

            -- Detour around mountains
            local nx = clamp(cx + mx, 0, CHUNK_SIZE - 1)
            local ny = clamp(cy + my, 0, CHUNK_SIZE - 1)
            if result[idx(nx, ny)] == "mountain" then
                if mx ~= 0 then mx, my = 0, (dy > 0 and 1 or -1)
                else             mx, my = (dx > 0 and 1 or -1), 0 end
                nx = clamp(cx + mx, 0, CHUNK_SIZE - 1)
                ny = clamp(cy + my, 0, CHUNK_SIZE - 1)
            end
            cx, cy = nx, ny
        end
    end
end

-- ─── Stage 4 – forest clusters ────────────────────────────────────────────────

local FOREST_PROTECTED = {
    city = true, city_entrance = true,
    water = true, mountain = true, river = true,
}

local function stage_forest(result, rng)
    local num_clusters = rng:random_range_u32(3, 8)

    for _ = 1, num_clusters do
        local cx     = rng:random_range_u32(0, CHUNK_SIZE - 1)
        local cy     = rng:random_range_u32(0, CHUNK_SIZE - 1)
        local radius = rng:random_range_u32(3, 8)

        for ty = 0, CHUNK_SIZE - 1 do
            for tx = 0, CHUNK_SIZE - 1 do
                local d = dist2d(tx, ty, cx, cy)
                if d <= radius and result[idx(tx, ty)] == "meadow" then
                    if rng:random_bool(1.0 - (d / radius) * 0.8) then
                        place(result, tx, ty, "forest", FOREST_PROTECTED)
                    end
                end
            end
        end
    end
end

-- ─── Stage 5 – mountain ridges ────────────────────────────────────────────────

local MOUNTAIN_OVERWRITABLE = { meadow = true, forest = true }
local MOUNTAIN_PROTECTED    = { city = true, city_entrance = true, water = true, river = true }

local function paint_ridge_segment(result, rng, x1, y1, x2, y2, hw)
    local sdx = x2 - x1
    local sdy = y2 - y1
    local slen = math.sqrt(sdx * sdx + sdy * sdy)
    if slen < 0.001 then return end

    local min_x = math.max(0, math.floor(math.min(x1, x2)) - hw - 1)
    local max_x = math.min(CHUNK_SIZE - 1, math.ceil(math.max(x1, x2)) + hw + 1)
    local min_y = math.max(0, math.floor(math.min(y1, y2)) - hw - 1)
    local max_y = math.min(CHUNK_SIZE - 1, math.ceil(math.max(y1, y2)) + hw + 1)

    for ty = min_y, max_y do
        for tx = min_x, max_x do
            local px = tx - x1
            local py = ty - y1
            local t  = clamp((px * sdx + py * sdy) / (slen * slen), 0.0, 1.0)
            local qx = x1 + t * sdx - tx
            local qy = y1 + t * sdy - ty
            local d  = math.sqrt(qx * qx + qy * qy)

            if d <= hw then
                local i = idx(tx, ty)
                if MOUNTAIN_OVERWRITABLE[result[i]] and not MOUNTAIN_PROTECTED[result[i]] then
                    if rng:random_bool(1.0 - (d / hw) * 0.5) then
                        result[i] = "mountain"
                    end
                end
            end
        end
    end
end

local function stage_mountains(result, rng)
    local num_ridges = rng:random_range_u32(1, 4)

    for _ = 1, num_ridges do
        local num_pts = rng:random_range_u32(3, 6)
        local hw      = rng:random_range_u32(1, 4)
        local px, py  = {}, {}
        px[1] = rng:random_range_u32(0, CHUNK_SIZE - 1)
        py[1] = rng:random_range_u32(0, CHUNK_SIZE - 1)

        for i = 2, num_pts do
            local step  = rng:random_range_u32(8, 17)
            local angle = rng:next_f64() * 2.0 * math.pi
            px[i] = clamp(px[i-1] + math.floor(step * math.cos(angle) + 0.5), 0, CHUNK_SIZE - 1)
            py[i] = clamp(py[i-1] + math.floor(step * math.sin(angle) + 0.5), 0, CHUNK_SIZE - 1)
        end

        for i = 1, num_pts - 1 do
            paint_ridge_segment(result, rng, px[i], py[i], px[i+1], py[i+1], hw)
        end
    end
end

-- ─── Stage 6 – roads ──────────────────────────────────────────────────────────

local ROAD_BLOCKED = {
    city = true, city_entrance = true,
    water = true, mountain = true, river = true,
}

--- Entry/exit point on `edge`, kept in the centre third of the edge.
local function road_edge_point(rng, edge)
    local lo  = math.floor(CHUNK_SIZE / 3)
    local hi  = math.floor(2 * CHUNK_SIZE / 3)
    local pos = random_edge_anchor(rng, lo, hi)
    if edge == 0 then return pos, 0
    elseif edge == 1 then return CHUNK_SIZE - 1, pos
    elseif edge == 2 then return pos, CHUNK_SIZE - 1
    else             return 0, pos
    end
end

local function stage_roads(result, rng)
    local roll = rng:next_f64()
    local num_roads = roll < 0.30 and 0 or (roll < 0.75 and 1 or 2)

    for _ = 1, num_roads do
        local se = rng:random_range_u32(0, 3)
        local ee = rng:random_bool(0.7) and ((se + 2) % 4)
                   or ((se + rng:random_range_u32(1, 3)) % 4)

        local cx, cy = road_edge_point(rng, se)
        local ex, ey = road_edge_point(rng, ee)

        for _ = 1, CHUNK_SIZE * 4 do
            if not ROAD_BLOCKED[result[idx(cx, cy)]] then
                result[idx(cx, cy)] = "road"
            end

            if (ee == 0 and cy <= 0) or (ee == 2 and cy >= CHUNK_SIZE - 1)
            or (ee == 1 and cx >= CHUNK_SIZE - 1) or (ee == 3 and cx <= 0) then
                break
            end

            local dx = ex - cx
            local dy = ey - cy
            if math.abs(dx) + math.abs(dy) == 0 then break end

            -- Primary move (85 % direct, 15 % meander)
            local mx, my
            if rng:random_bool(0.85) then
                if math.abs(dx) >= math.abs(dy) then mx, my = (dx > 0 and 1 or -1), 0
                else                                  mx, my = 0, (dy > 0 and 1 or -1) end
            else
                if math.abs(dx) >= math.abs(dy) then
                    mx, my = 0, (dy ~= 0 and (dy > 0 and 1 or -1) or (rng:random_bool(0.5) and 1 or -1))
                else
                    mx, my = (dx ~= 0 and (dx > 0 and 1 or -1) or (rng:random_bool(0.5) and 1 or -1)), 0
                end
            end

            -- Navigate around blockers: try alternates in priority order
            local alts = { {mx, my}, {my, mx}, {-my, -mx}, {-mx, -my} }
            for _, mv in ipairs(alts) do
                local nx = clamp(cx + mv[1], 0, CHUNK_SIZE - 1)
                local ny = clamp(cy + mv[2], 0, CHUNK_SIZE - 1)
                if not ROAD_BLOCKED[result[idx(nx, ny)]] then
                    cx, cy = nx, ny
                    break
                end
            end
        end
    end
end

-- ─── Stage 7 – city block ─────────────────────────────────────────────────────
--
-- Places one 3×3 city block per chunk.  Up to 20 candidate positions are tried;
-- each is accepted only when the 3×3 block plus a 1-tile margin is completely
-- free of water, river, and mountain.  If no valid position is found the stage
-- is silently skipped.
--
-- Layout:
--   rows by..by+1  — all "city" (6 tiles)
--   row  by+2      — "city_entrance" at bx, then "city" × 2
-- Entrance neighbours outside the block are forced to "meadow" or "road".
-- Placed BEFORE resources so the resource stage respects the exclusion zone.

local CITY_BLOCKED = { water = true, river = true, mountain = true }

local function city_area_clear(result, bx, by)
    -- Check 3-wide × 3-tall block + 1-tile margin on all sides (5×4 scan)
    for dy = -1, 3 do
        for dx = -1, 3 do
            local nx, ny = bx + dx, by + dy
            if nx >= 0 and nx < CHUNK_SIZE and ny >= 0 and ny < CHUNK_SIZE then
                if CITY_BLOCKED[result[idx(nx, ny)]] then return false end
            end
        end
    end
    return true
end

local function stage_city(result, rng)
    local bx, by
    for _ = 1, 20 do
        local tx = rng:random_range_u32(1, 28)
        local ty = rng:random_range_u32(1, 26)
        if city_area_clear(result, tx, ty) then
            bx, by = tx, ty
            break
        end
    end
    if not bx then return end  -- no valid position found

    -- Top two rows: all city
    for dy = 0, 1 do
        for dx = 0, 2 do
            result[idx(bx + dx, by + dy)] = "city"
        end
    end
    -- Bottom row: entrance at left, city on right two
    result[idx(bx,     by + 2)] = "city_entrance"
    result[idx(bx + 1, by + 2)] = "city"
    result[idx(bx + 2, by + 2)] = "city"

    -- Force entrance neighbours (outside the 3×3 block) to meadow or road
    local nb = {
        { bx - 1, by + 2 }, { bx + 3, by + 2 },
        { bx,     by + 3 }, { bx + 1, by + 3 }, { bx + 2, by + 3 },
    }
    for _, p in ipairs(nb) do
        local nx, ny = p[1], p[2]
        if nx >= 0 and nx < CHUNK_SIZE and ny >= 0 and ny < CHUNK_SIZE then
            result[idx(nx, ny)] = rng:random_bool(0.7) and "meadow" or "road"
        end
    end
end

-- ─── Stage 8 – resources ──────────────────────────────────────────────────────
--
-- Rules:
--   • The resource tile and all 8 neighbours (3×3 area) must be "meadow".
--   • Resources must not be within SAFE_RADIUS of any settlement tile.
--   • Resources must be at least MIN_SPACING apart from each other.
--   • 1 gold mine per chunk + 4–7 generic resource deposits.

local RES_SETTLEMENT = { city = true, city_entrance = true, village = true }
local MIN_SPACING    = 4
local SAFE_RADIUS    = 2
local MAX_TRIES      = 120

--- Returns true when all 9 tiles of the 3×3 area centred on (tx,ty) are "meadow".
local function meadow_3x3(result, tx, ty)
    for dy = -1, 1 do
        for dx = -1, 1 do
            local nx, ny = tx + dx, ty + dy
            if nx < 0 or nx >= CHUNK_SIZE or ny < 0 or ny >= CHUNK_SIZE then
                return false
            end
            if result[idx(nx, ny)] ~= "meadow" then return false end
        end
    end
    return true
end

local function near_settlement(result, tx, ty)
    for dy = -SAFE_RADIUS, SAFE_RADIUS do
        for dx = -SAFE_RADIUS, SAFE_RADIUS do
            local nx, ny = tx + dx, ty + dy
            if nx >= 0 and nx < CHUNK_SIZE and ny >= 0 and ny < CHUNK_SIZE then
                if RES_SETTLEMENT[result[idx(nx, ny)]] then return true end
            end
        end
    end
    return false
end

local function spaced_ok(placed, tx, ty)
    for _, p in ipairs(placed) do
        if dist2d(tx, ty, p[1], p[2]) < MIN_SPACING then return false end
    end
    return true
end

local function try_place_resource(result, rng, placed, kind)
    for _ = 1, MAX_TRIES do
        -- Keep 1-tile margin so the 3×3 check never goes out of bounds
        local tx = rng:random_range_u32(1, CHUNK_SIZE - 2)
        local ty = rng:random_range_u32(1, CHUNK_SIZE - 2)
        if meadow_3x3(result, tx, ty)
            and not near_settlement(result, tx, ty)
            and spaced_ok(placed, tx, ty)
        then
            result[idx(tx, ty)] = kind
            placed[#placed + 1] = { tx, ty }
            return true
        end
    end
    return false
end

local function stage_resources(result, rng)
    local placed = {}
    try_place_resource(result, rng, placed, "gold")
    local num_res = rng:random_range_u32(4, 8)  -- [4, 7]
    for _ = 1, num_res do
        try_place_resource(result, rng, placed, "resource")
    end
end

-- ─── Entry point ──────────────────────────────────────────────────────────────

local function generate_chunk(rng, x, y, tiles)
    local result = {}
    stage_base(result, tiles)
    stage_water(result, rng)
    stage_rivers(result, rng)
    stage_forest(result, rng)
    stage_mountains(result, rng)
    stage_roads(result, rng)
    stage_city(result, rng)
    stage_resources(result, rng)
    return result
end

return generate_chunk

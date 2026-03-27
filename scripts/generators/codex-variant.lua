--- Codex terrain variant.
--
-- Distinct generator pipeline focused on broad landforms and connective tissue:
--   ridge with passes -> lake basin -> river -> forest belts -> city -> roads
--   -> optional POI -> resource nodes
--
-- The script accepts an optional 4th argument `tiles`. When present, those
-- tiles are copied first and then augmented by this generator.

local CHUNK_SIZE = 32
local TILE_COUNT = CHUNK_SIZE * CHUNK_SIZE

local function idx(tx, ty)
    return ty * CHUNK_SIZE + tx + 1
end

local function clamp(v, lo, hi)
    return math.max(lo, math.min(hi, v))
end

local function in_bounds(tx, ty)
    return tx >= 0 and tx < CHUNK_SIZE and ty >= 0 and ty < CHUNK_SIZE
end

local function dist(tx, ty, ox, oy)
    local dx = tx - ox
    local dy = ty - oy
    return math.sqrt(dx * dx + dy * dy)
end

local function manhattan(tx, ty, ox, oy)
    return math.abs(tx - ox) + math.abs(ty - oy)
end

local function point_segment_distance(px, py, ax, ay, bx, by)
    local dx = bx - ax
    local dy = by - ay
    local len2 = dx * dx + dy * dy
    if len2 < 0.0001 then
        return dist(px, py, ax, ay), 0.0
    end

    local t = clamp(((px - ax) * dx + (py - ay) * dy) / len2, 0.0, 1.0)
    local qx = ax + dx * t
    local qy = ay + dy * t
    return dist(px, py, qx, qy), t
end

local function copy_or_fill(result, tiles)
    if tiles ~= nil then
        for i = 1, TILE_COUNT do
            result[i] = tiles[i]
        end
        return
    end

    for i = 1, TILE_COUNT do
        result[i] = "meadow"
    end
end

local function paint_disc(result, cx, cy, radius, kind, allowed)
    local lo_x = math.max(0, math.floor(cx - radius))
    local hi_x = math.min(CHUNK_SIZE - 1, math.ceil(cx + radius))
    local lo_y = math.max(0, math.floor(cy - radius))
    local hi_y = math.min(CHUNK_SIZE - 1, math.ceil(cy + radius))

    for ty = lo_y, hi_y do
        for tx = lo_x, hi_x do
            if dist(tx, ty, cx, cy) <= radius then
                local i = idx(tx, ty)
                if allowed[result[i]] then
                    result[i] = kind
                end
            end
        end
    end
end

local function clear_disc(result, cx, cy, radius)
    local lo_x = math.max(0, math.floor(cx - radius))
    local hi_x = math.min(CHUNK_SIZE - 1, math.ceil(cx + radius))
    local lo_y = math.max(0, math.floor(cy - radius))
    local hi_y = math.min(CHUNK_SIZE - 1, math.ceil(cy + radius))

    for ty = lo_y, hi_y do
        for tx = lo_x, hi_x do
            if dist(tx, ty, cx, cy) <= radius then
                local i = idx(tx, ty)
                if result[i] ~= "city" and result[i] ~= "city_entrance" then
                    result[i] = "meadow"
                end
            end
        end
    end
end

local function near_any(result, tx, ty, radius, set)
    for dy = -radius, radius do
        for dx = -radius, radius do
            local nx = tx + dx
            local ny = ty + dy
            if in_bounds(nx, ny) and set[result[idx(nx, ny)]] then
                return true
            end
        end
    end
    return false
end

local MEADOWISH = { meadow = true, forest = true, road = true }
local RIVER_WRITABLE = { meadow = true, forest = true, road = true }
local ROAD_BLOCKED = { city = true, city_entrance = true, water = true, mountain = true }
local POI_CLEAR = { meadow = true, forest = true, road = true, bridge = true }
local WET = { water = true, river = true, bridge = true }
local SETTLEMENT = { city = true, city_entrance = true, village = true, merchant = true }
local RESOURCE_KEEP_OUT = {
    city = true,
    city_entrance = true,
    village = true,
    merchant = true,
    ruins = true,
    water = true,
    river = true,
    mountain = true,
}

local function stage_ridge(result, rng, state)
    local start_side = rng:random_range_u32(0, 4)
    local end_side = (start_side + rng:random_range_u32(1, 4)) % 4

    local function side_point(side)
        local pos = rng:random_range_u32(4, CHUNK_SIZE - 4)
        if side == 0 then return pos, 0
        elseif side == 1 then return CHUNK_SIZE - 1, pos
        elseif side == 2 then return pos, CHUNK_SIZE - 1
        else return 0, pos end
    end

    local ax, ay = side_point(start_side)
    local bx, by = side_point(end_side)
    local width = rng:random_range_u32(2, 5)
    local waviness = rng:next_f64() * 2.5 + 1.5
    local phase = rng:next_f64() * math.pi * 2.0

    for ty = 0, CHUNK_SIZE - 1 do
        for tx = 0, CHUNK_SIZE - 1 do
            local d, t = point_segment_distance(tx, ty, ax, ay, bx, by)
            local i = idx(tx, ty)
            local local_width = width + math.sin(t * math.pi * waviness + phase) * 1.2
            local texture = math.sin((tx + 1) * 0.53 + phase) + math.cos((ty + 1) * 0.37 - phase)
            if d <= local_width and texture > -0.45 and MEADOWISH[result[i]] then
                result[i] = "mountain"
            end
        end
    end

    state.passes = {}
    local pass_count = rng:random_range_u32(1, 3)
    for p = 1, pass_count do
        local t = (p / (pass_count + 1)) + (rng:next_f64() - 0.5) * 0.18
        t = clamp(t, 0.12, 0.88)
        local px = math.floor(ax + (bx - ax) * t + 0.5)
        local py = math.floor(ay + (by - ay) * t + 0.5)
        clear_disc(result, px, py, rng:random_range_u32(2, 4))
        state.passes[#state.passes + 1] = { px, py }
    end
end

local function stage_lake(result, rng, state)
    local cx = rng:random_range_u32(6, CHUNK_SIZE - 6)
    local cy = rng:random_range_u32(6, CHUNK_SIZE - 6)
    local rx = rng:random_range_u32(4, 8)
    local ry = rng:random_range_u32(3, 7)
    local skew = rng:next_f64() * 0.8 - 0.4
    local phase = rng:next_f64() * math.pi * 2.0

    for ty = 0, CHUNK_SIZE - 1 do
        for tx = 0, CHUNK_SIZE - 1 do
            local dx = (tx - cx) / rx
            local dy = (ty - cy) / ry
            local ripple = math.sin((tx + ty) * 0.35 + phase) * 0.11
            local basin = dx * dx + (dy + dx * skew) * (dy + dx * skew) + ripple
            local i = idx(tx, ty)
            if basin <= 1.0 and result[i] ~= "mountain" then
                result[i] = "water"
            end
        end
    end

    paint_disc(result, cx + skew * 3.0, cy + 1.0, rng:random_range_u32(2, 4), "water", MEADOWISH)
    state.lake = { cx = cx, cy = cy }
end

local function stage_river(result, rng, state)
    local sx, sy
    if #state.passes > 0 then
        local pick = state.passes[rng:random_range_u32(1, #state.passes + 1)]
        sx, sy = pick[1], pick[2]
    else
        sx = rng:random_range_u32(5, CHUNK_SIZE - 5)
        sy = rng:random_range_u32(5, CHUNK_SIZE - 5)
    end

    local edge = rng:random_range_u32(0, 4)
    local tx
    local ty
    if edge == 0 then
        tx = rng:random_range_u32(4, CHUNK_SIZE - 4)
        ty = 0
    elseif edge == 1 then
        tx = CHUNK_SIZE - 1
        ty = rng:random_range_u32(4, CHUNK_SIZE - 4)
    elseif edge == 2 then
        tx = rng:random_range_u32(4, CHUNK_SIZE - 4)
        ty = CHUNK_SIZE - 1
    else
        tx = 0
        ty = rng:random_range_u32(4, CHUNK_SIZE - 4)
    end

    if state.lake and rng:random_bool(0.55) then
        tx = state.lake.cx
        ty = state.lake.cy
    end

    local cx = sx
    local cy = sy
    for _ = 1, CHUNK_SIZE * 6 do
        local i = idx(cx, cy)
        if RIVER_WRITABLE[result[i]] then
            result[i] = "river"
        end

        if rng:random_bool(0.28) then
            for _, step in ipairs({ { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }) do
                local nx = cx + step[1]
                local ny = cy + step[2]
                if in_bounds(nx, ny) and RIVER_WRITABLE[result[idx(nx, ny)]] then
                    result[idx(nx, ny)] = "river"
                end
            end
        end

        if manhattan(cx, cy, tx, ty) <= 1 then
            break
        end

        local best_score = nil
        local best_next = nil
        for _, step in ipairs({ { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }) do
            local nx = clamp(cx + step[1], 0, CHUNK_SIZE - 1)
            local ny = clamp(cy + step[2], 0, CHUNK_SIZE - 1)
            local tile = result[idx(nx, ny)]
            local penalty = 0.0

            if tile == "mountain" or tile == "city" or tile == "city_entrance" then
                penalty = penalty + 100.0
            elseif tile == "water" and not (nx == tx and ny == ty) then
                penalty = penalty + 3.0
            end

            local score = manhattan(nx, ny, tx, ty) + penalty + rng:next_f64() * 0.45
            if best_score == nil or score < best_score then
                best_score = score
                best_next = { nx, ny }
            end
        end

        if best_next == nil then
            break
        end

        cx = best_next[1]
        cy = best_next[2]
    end
end

local function stage_forests(result, rng)
    local phase = rng:next_f64() * math.pi * 2.0
    for ty = 0, CHUNK_SIZE - 1 do
        for tx = 0, CHUNK_SIZE - 1 do
            local i = idx(tx, ty)
            if result[i] == "meadow" then
                local score = 0.03
                if near_any(result, tx, ty, 2, WET) then
                    score = score + 0.22
                end
                if near_any(result, tx, ty, 3, { mountain = true }) then
                    score = score + 0.32
                end
                local waves = math.sin(tx * 0.41 + phase) + math.cos(ty * 0.33 - phase)
                if waves > 0.65 then
                    score = score + 0.18
                end
                if rng:random_bool(math.min(score, 0.82)) then
                    result[i] = "forest"
                end
            end
        end
    end
end

local function city_site_clear(result, bx, by)
    for dy = -1, 3 do
        for dx = -1, 3 do
            local nx = bx + dx
            local ny = by + dy
            if not in_bounds(nx, ny) then
                return false
            end
            local kind = result[idx(nx, ny)]
            if kind == "mountain" or kind == "water" or kind == "river" then
                return false
            end
        end
    end
    return true
end

local function stage_city(result, rng, state)
    local candidates = {}
    for by = 2, CHUNK_SIZE - 5 do
        for bx = 2, CHUNK_SIZE - 5 do
            if city_site_clear(result, bx, by) then
                local entrance_x = bx
                local entrance_y = by + 2
                local score = 0
                if near_any(result, entrance_x, entrance_y, 4, WET) then
                    score = score + 2
                end
                if near_any(result, entrance_x, entrance_y, 4, { mountain = true }) then
                    score = score + 2
                end
                score = score + manhattan(entrance_x, entrance_y, 16, 18) * 0.15
                candidates[#candidates + 1] = { bx, by, score }
            end
        end
    end

    if #candidates == 0 then
        return
    end

    local best = candidates[1]
    for i = 2, #candidates do
        local c = candidates[i]
        if c[3] < best[3] or (c[3] == best[3] and rng:random_bool(0.5)) then
            best = c
        end
    end

    local bx = best[1]
    local by = best[2]
    for dy = 0, 1 do
        for dx = 0, 2 do
            result[idx(bx + dx, by + dy)] = "city"
        end
    end
    result[idx(bx, by + 2)] = "city_entrance"
    result[idx(bx + 1, by + 2)] = "city"
    result[idx(bx + 2, by + 2)] = "city"

    for _, p in ipairs({
        { bx, by + 3 },
        { bx + 1, by + 3 },
        { bx + 2, by + 3 },
        { bx - 1, by + 2 },
        { bx + 3, by + 2 },
    }) do
        local px = p[1]
        local py = p[2]
        if in_bounds(px, py) and result[idx(px, py)] ~= "river" then
            result[idx(px, py)] = "meadow"
        end
    end

    state.city = { bx = bx, by = by, entrance_x = bx, entrance_y = by + 2 }
end

local function trace_road(result, rng, sx, sy, tx, ty)
    local cx = sx
    local cy = sy
    for _ = 1, CHUNK_SIZE * 8 do
        local current = result[idx(cx, cy)]
        if current == "river" then
            result[idx(cx, cy)] = "bridge"
        elseif current ~= "city" and current ~= "city_entrance" and current ~= "water" and current ~= "mountain" then
            result[idx(cx, cy)] = "road"
        end

        if cx == tx and cy == ty then
            break
        end

        local best_score = nil
        local best_next = nil
        for _, step in ipairs({ { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }) do
            local nx = clamp(cx + step[1], 0, CHUNK_SIZE - 1)
            local ny = clamp(cy + step[2], 0, CHUNK_SIZE - 1)
            local tile = result[idx(nx, ny)]
            local penalty = 0.0

            if ROAD_BLOCKED[tile] then
                penalty = penalty + 100.0
            elseif tile == "forest" then
                penalty = penalty + 0.45
            elseif tile == "river" then
                penalty = penalty + 0.20
            end

            local score = manhattan(nx, ny, tx, ty) + penalty + rng:next_f64() * 0.35
            if best_score == nil or score < best_score then
                best_score = score
                best_next = { nx, ny }
            end
        end

        if best_next == nil then
            break
        end
        cx = best_next[1]
        cy = best_next[2]
    end
end

local function stage_roads(result, rng, state)
    if not state.city then
        return
    end

    local ex = state.city.entrance_x
    local ey = state.city.entrance_y + 1
    if in_bounds(ex, ey) and result[idx(ex, ey)] == "river" then
        result[idx(ex, ey)] = "bridge"
    elseif in_bounds(ex, ey) and result[idx(ex, ey)] ~= "water" and result[idx(ex, ey)] ~= "mountain" then
        result[idx(ex, ey)] = "road"
    end

    local gates = {}
    gates[1] = { rng:random_range_u32(8, CHUNK_SIZE - 8), 0 }
    gates[2] = { CHUNK_SIZE - 1, rng:random_range_u32(8, CHUNK_SIZE - 8) }
    if rng:random_bool(0.5) then
        gates[2] = { rng:random_range_u32(8, CHUNK_SIZE - 8), CHUNK_SIZE - 1 }
    end

    for _, gate in ipairs(gates) do
        trace_road(result, rng, state.city.entrance_x, state.city.entrance_y, gate[1], gate[2])
    end
end

local function place_poi(result, rng, kind, tries, predicate)
    for _ = 1, tries do
        local tx = rng:random_range_u32(1, CHUNK_SIZE - 1)
        local ty = rng:random_range_u32(1, CHUNK_SIZE - 1)
        local tile = result[idx(tx, ty)]
        if POI_CLEAR[tile] and predicate(tx, ty) then
            result[idx(tx, ty)] = kind
            return true
        end
    end
    return false
end

local function stage_poi(result, rng)
    place_poi(result, rng, "village", 80, function(tx, ty)
        return near_any(result, tx, ty, 1, { road = true, bridge = true })
            and not near_any(result, tx, ty, 2, SETTLEMENT)
    end)

    place_poi(result, rng, "merchant", 80, function(tx, ty)
        return near_any(result, tx, ty, 1, { road = true, bridge = true })
            and near_any(result, tx, ty, 4, SETTLEMENT)
            and not near_any(result, tx, ty, 1, WET)
    end)

    place_poi(result, rng, "ruins", 80, function(tx, ty)
        return near_any(result, tx, ty, 1, { forest = true, mountain = true })
            and not near_any(result, tx, ty, 2, SETTLEMENT)
    end)
end

local function meadow_ring_clear(result, tx, ty)
    for dy = -1, 1 do
        for dx = -1, 1 do
            local nx = tx + dx
            local ny = ty + dy
            if not in_bounds(nx, ny) then
                return false
            end
            if result[idx(nx, ny)] ~= "meadow" then
                return false
            end
        end
    end
    return true
end

local function spaced_from(placed, tx, ty, min_dist)
    for _, p in ipairs(placed) do
        if dist(tx, ty, p[1], p[2]) < min_dist then
            return false
        end
    end
    return true
end

local function stage_resources(result, rng)
    local placed = {}

    local function try_resource(kind, prefer)
        for _ = 1, 120 do
            local tx = rng:random_range_u32(1, CHUNK_SIZE - 1)
            local ty = rng:random_range_u32(1, CHUNK_SIZE - 1)
            if meadow_ring_clear(result, tx, ty)
                and spaced_from(placed, tx, ty, 4.0)
                and not near_any(result, tx, ty, 2, RESOURCE_KEEP_OUT)
                and prefer(tx, ty)
            then
                result[idx(tx, ty)] = kind
                placed[#placed + 1] = { tx, ty }
                return true
            end
        end
        return false
    end

    try_resource("gold", function(tx, ty)
        return near_any(result, tx, ty, 3, { mountain = true })
    end)

    local total = rng:random_range_u32(4, 7)
    for _ = 1, total do
        try_resource("resource", function(tx, ty)
            return near_any(result, tx, ty, 3, { forest = true, road = true, bridge = true })
        end)
    end
end

local function generate_chunk(rng, x, y, tiles)
    local result = {}
    local state = { chunk_x = x, chunk_y = y, passes = {} }

    copy_or_fill(result, tiles)
    stage_ridge(result, rng, state)
    stage_lake(result, rng, state)
    stage_river(result, rng, state)
    stage_forests(result, rng)
    stage_city(result, rng, state)
    stage_roads(result, rng, state)
    stage_poi(result, rng)
    stage_resources(result, rng)

    return result
end

return generate_chunk

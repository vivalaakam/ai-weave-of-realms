--- Mountain ridge generator.
--
-- Pipeline stage that overlays mountain ridges onto base tiles.
-- Receives base tiles as 4th argument and carves mountain corridors
-- over meadow and forest tiles only.
--
-- ## Algorithm
-- - Generates 1–3 mountain ridges
-- - Each ridge is a random polyline of 3–5 waypoints with step ~8–16 tiles
-- - Along the polyline, a mountain corridor of width 1–3 tiles is painted
-- - Mountain probability decreases from the ridge centre: p = 1.0 - (r / width) * 0.5
-- - Only overwrites "meadow" and "forest" tiles (forest at mountain foothills)
-- - Protected: "city", "city_entrance", "water", "river"
--
-- @param rng   SeededRng userdata — deterministic RNG seeded per chunk.
--              Methods: next_f64(), random_range_u32(lo, hi), random_bool(probability)
-- @param x     Chunk column index (0-based).
-- @param y     Chunk row index (0-based).
-- @param tiles Base tile table from the previous pipeline stage (1-indexed, 1024 entries).
--              If nil, all tiles start as "meadow".
-- @return      table of 1024 strings with mountain ridges applied.

local CHUNK_SIZE = 32

-- Tiles that must not be overwritten by mountains
local PROTECTED = {
    city          = true,
    city_entrance = true,
    water         = true,
    river         = true,
}

-- Tiles that mountains are allowed to overwrite
local OVERWRITABLE = {
    meadow = true,
    forest = true,
}

local function tile_index(lx, ly)
    return ly * CHUNK_SIZE + lx + 1
end

--- Paints mountain tiles along a segment from (x1,y1) to (x2,y2) with given half-width.
local function paint_segment(result, rng, x1, y1, x2, y2, half_width)
    -- Bounding box to iterate (clamped to chunk)
    local min_x = math.max(0, math.floor(math.min(x1, x2)) - half_width - 1)
    local max_x = math.min(CHUNK_SIZE - 1, math.ceil(math.max(x1, x2)) + half_width + 1)
    local min_y = math.max(0, math.floor(math.min(y1, y2)) - half_width - 1)
    local max_y = math.min(CHUNK_SIZE - 1, math.ceil(math.max(y1, y2)) + half_width + 1)

    -- Segment direction vector
    local seg_dx = x2 - x1
    local seg_dy = y2 - y1
    local seg_len = math.sqrt(seg_dx * seg_dx + seg_dy * seg_dy)

    if seg_len < 0.001 then return end

    for ty = min_y, max_y do
        for tx = min_x, max_x do
            -- Distance from point (tx, ty) to the line segment
            local px = tx - x1
            local py = ty - y1
            local t = (px * seg_dx + py * seg_dy) / (seg_len * seg_len)
            t = math.max(0.0, math.min(1.0, t))
            local closest_x = x1 + t * seg_dx
            local closest_y = y1 + t * seg_dy
            local dist_x = tx - closest_x
            local dist_y = ty - closest_y
            local dist = math.sqrt(dist_x * dist_x + dist_y * dist_y)

            if dist <= half_width then
                local idx = tile_index(tx, ty)
                local current = result[idx]

                if OVERWRITABLE[current] and not PROTECTED[current] then
                    local p = 1.0 - (dist / half_width) * 0.5
                    if rng:random_bool(p) then
                        result[idx] = "mountain"
                    end
                end
            end
        end
    end
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

    -- Number of ridges: 1–3
    local num_ridges = rng:random_range_u32(1, 4)  -- [1, 3] inclusive

    for _ = 1, num_ridges do
        -- Number of waypoints in this ridge: 3–5
        local num_points = rng:random_range_u32(3, 6)  -- [3, 5] inclusive

        -- Corridor half-width: 1–3 tiles (half-width = floor(width/2))
        local half_width = rng:random_range_u32(1, 4)  -- [1, 3]

        -- Generate waypoints; first point is random within chunk
        local pts_x = {}
        local pts_y = {}
        pts_x[1] = rng:random_range_u32(0, CHUNK_SIZE - 1)
        pts_y[1] = rng:random_range_u32(0, CHUNK_SIZE - 1)

        for i = 2, num_points do
            -- Each subsequent point steps 8–16 tiles in a random direction
            local step = rng:random_range_u32(8, 17)  -- [8, 16]
            local angle = rng:next_f64() * 2.0 * math.pi
            local nx = pts_x[i - 1] + math.floor(step * math.cos(angle) + 0.5)
            local ny = pts_y[i - 1] + math.floor(step * math.sin(angle) + 0.5)
            -- Clamp to chunk bounds
            pts_x[i] = math.max(0, math.min(CHUNK_SIZE - 1, nx))
            pts_y[i] = math.max(0, math.min(CHUNK_SIZE - 1, ny))
        end

        -- Paint along each segment of the polyline
        for i = 1, num_points - 1 do
            paint_segment(result, rng,
                pts_x[i], pts_y[i],
                pts_x[i + 1], pts_y[i + 1],
                half_width)
        end
    end

    return result
end

return generate_chunk

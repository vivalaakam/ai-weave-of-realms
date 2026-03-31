--- Enemy spawn rules.
--
-- Determines enemy spawn positions and stats based on map layout.
-- Enemies are distributed across map quadrants to ensure coverage.
--
-- @param map  Table: map.chunks_wide, map.chunks_tall, map.tiles (flat array)
-- @return     Array of enemy descriptors: { id, x, y, hp, atk, def, spd, mov }

-- Enemy type definitions
local ENEMY_TYPES = {
    grunt = { hp = 30,  atk = 10, def = 5,  spd = 5,  mov = 3 },
    warrior = { hp = 50, atk = 15, def = 8,  spd = 6,  mov = 3 },
    ranger = { hp = 35,  atk = 18, def = 4,  spd = 8,  mov = 4 },
    boss = { hp = 100,  atk = 25, def = 12, spd = 4,  mov = 2 },
}

-- Tiles enemies can spawn on
local SPAWNABLE = {
    meadow = true, forest = true, road = true, bridge = true,
}

-- Tiles to avoid (points of interest)
local POI_TILES = {
    city = true, city_entrance = true, village = true, merchant = true,
    ruins = true, gold = true, resource = true,
}

local function tile_at(map, x, y)
    local w = map.width or (map.chunks_wide * 32)
    local idx = y * w + x + 1
    if idx < 1 or idx > #map.tiles then
        return nil
    end
    return map.tiles[idx]
end

local function find_player_start(map)
    -- Find city_entrance first, then road, then any passable
    local w = map.width or (map.chunks_wide * 32)
    for idx, kind in ipairs(map.tiles) do
        if kind == "city_entrance" then
            local x = (idx - 1) % w
            local y = math.floor((idx - 1) / w)
            return x, y
        end
    end

    for idx, kind in ipairs(map.tiles) do
        if kind == "road" then
            local x = (idx - 1) % w
            local y = math.floor((idx - 1) / w)
            return x, y
        end
    end

    return nil, nil
end

local function distance(x1, y1, x2, y2)
    local dx = x1 - x2
    local dy = y1 - y2
    return math.sqrt(dx * dx + dy * dy)
end

-- Map is divided into zones for guaranteed enemy distribution
local function get_zone(x, y, map_w, map_h)
    local half_w = map_w / 2
    local half_h = map_h / 2
    
    if x < half_w and y < half_h then return 1 end  -- top-left
    if x >= half_w and y < half_h then return 2 end -- top-right
    if x < half_w and y >= half_h then return 3 end -- bottom-left
    return 4 -- bottom-right
end

local function is_valid_spawn(tx, ty, placed, map_w, map_h, map)
    -- Check bounds
    if tx < 0 or tx >= map_w or ty < 0 or ty >= map_h then
        return false
    end

    -- Check tile type
    local tile = tile_at({ tiles = map.tiles, chunks_wide = map.chunks_wide, width = map_w }, tx, ty)
    if not tile or not SPAWNABLE[tile] then
        return false
    end

    -- Check distance from other placed enemies (minimum 5 tiles apart)
    for _, p in ipairs(placed) do
        if distance(tx, ty, p.x, p.y) < 5 then
            return false
        end
    end

    -- Avoid tiles too close to POIs (3-tile radius)
    for dy = -3, 3 do
        for dx = -3, 3 do
            local nx, ny = tx + dx, ty + dy
            if nx >= 0 and nx < map_w and ny >= 0 and ny < map_h then
                local nt = tile_at({ tiles = map.tiles, chunks_wide = map.chunks_wide, width = map_w }, nx, ny)
                if nt and POI_TILES[nt] then
                    return false
                end
            end
        end
    end

    return true
end

-- Count spawnable tiles in a zone
local function count_spawnable_in_zone(map, zone, map_w, map_h)
    local half_w = map_w / 2
    local half_h = map_h / 2
    
    local x_min, x_max, y_min, y_max
    if zone == 1 then x_min, x_max, y_min, y_max = 0, half_w - 1, 0, half_h - 1
    elseif zone == 2 then x_min, x_max, y_min, y_max = half_w, map_w - 1, 0, half_h - 1
    elseif zone == 3 then x_min, x_max, y_min, y_max = 0, half_w - 1, half_h, map_h - 1
    else x_min, x_max, y_min, y_max = half_w, map_w - 1, half_h, map_h - 1 end
    
    local count = 0
    for y = math.floor(y_min), math.floor(y_max) do
        for x = math.floor(x_min), math.floor(x_max) do
            local tile = tile_at({ tiles = map.tiles, chunks_wide = map.chunks_wide, width = map_w }, x, y)
            if tile and SPAWNABLE[tile] then
                count = count + 1
            end
        end
    end
    return count
end

-- Try to place enemy in a specific zone
local function try_place_in_zone(map, zone, placed, map_w, map_h, max_attempts)
    local half_w = map_w / 2
    local half_h = map_h / 2
    
    local x_min, x_max, y_min, y_max
    if zone == 1 then x_min, x_max, y_min, y_max = 0, half_w - 1, 0, half_h - 1
    elseif zone == 2 then x_min, x_max, y_min, y_max = half_w, map_w - 1, 0, half_h - 1
    elseif zone == 3 then x_min, x_max, y_min, y_max = 0, half_w - 1, half_h, map_h - 1
    else x_min, x_max, y_min, y_max = half_w, map_w - 1, half_h, map_h - 1 end
    
    -- Check if zone has spawnable tiles
    local spawnable = count_spawnable_in_zone(map, zone, map_w, map_h)
    if spawnable < 5 then
        return nil -- Zone too small or no spawnable tiles
    end
    
    for _ = 1, max_attempts do
        local tx = math.random(math.floor(x_min), math.floor(x_max))
        local ty = math.random(math.floor(y_min), math.floor(y_max))
        
        if is_valid_spawn(tx, ty, placed, map_w, map_h, map) then
            return { x = tx, y = ty }
        end
    end
    
    return nil
end

return function(map)
    local map_w = map.width or (map.chunks_wide * 32)
    local map_h = map.height or (map.chunks_tall * 32)
    local enemies = {}
    local placed = {}
    local id_counter = 1
    
    -- Target: 5 enemies minimum, one in each zone
    local MIN_ENEMIES = 5
    local MAX_ENEMIES = 10
    
    -- Distribute enemies across zones
    local zones = { 1, 2, 3, 4 }
    
    -- Debug: log zone counts
    local zone_counts = {}
    for z = 1, 4 do
        zone_counts[z] = count_spawnable_in_zone(map, z, map_w, map_h)
    end
    
    -- First pass: try to place at least one enemy in each zone
    for _, zone in ipairs(zones) do
        if #enemies < MAX_ENEMIES then
            local pos = try_place_in_zone(map, zone, placed, map_w, map_h, 200)
            if pos then
                -- Determine enemy type based on zone distance from center
                local center_dist = distance(pos.x, pos.y, map_w / 2, map_h / 2)
                local max_dist = math.sqrt((map_w / 2) ^ 2 + (map_h / 2) ^ 2)
                local dist_ratio = center_dist / max_dist
                
                local enemy_type
                local roll = math.random()
                
                if dist_ratio > 0.6 and roll < 0.25 then
                    enemy_type = "boss"
                elseif dist_ratio > 0.4 and roll < 0.5 then
                    enemy_type = "warrior"
                elseif roll < 0.3 then
                    enemy_type = "ranger"
                else
                    enemy_type = "grunt"
                end
                
                local stats = ENEMY_TYPES[enemy_type]
                enemies[#enemies + 1] = {
                    id = id_counter,
                    x = pos.x,
                    y = pos.y,
                    hp = stats.hp,
                    atk = stats.atk,
                    def = stats.def,
                    spd = stats.spd,
                    mov = stats.mov,
                }
                placed[#placed + 1] = pos
                id_counter = id_counter + 1
            end
        end
    end
    
    -- Second pass: fill if we have fewer than MIN_ENEMIES
    while #enemies < MIN_ENEMIES and #enemies < MAX_ENEMIES do
        -- Pick zone with fewest enemies
        local min_zone = 1
        local min_count = 999
        for _, p in ipairs(placed) do
            local zone = get_zone(p.x, p.y, map_w, map_h)
            if zone and zone_counts[zone] then
                zone_counts[zone] = zone_counts[zone] - 1
            end
        end
        
        for z = 1, 4 do
            if zone_counts[z] and zone_counts[z] < min_count and zone_counts[z] > 0 then
                min_count = zone_counts[zone]
                min_zone = z
            end
        end
        
        local pos = try_place_in_zone(map, min_zone, placed, map_w, map_h, 100)
        if not pos then
            -- Fallback: try any zone
            for _, zone in ipairs(zones) do
                pos = try_place_in_zone(map, zone, placed, map_w, map_h, 50)
                if pos then break end
            end
        end
        
        if pos then
            local enemy_type = math.random() < 0.7 and "grunt" or "warrior"
            local stats = ENEMY_TYPES[enemy_type]
            enemies[#enemies + 1] = {
                id = id_counter,
                x = pos.x,
                y = pos.y,
                hp = stats.hp,
                atk = stats.atk,
                def = stats.def,
                spd = stats.spd,
                mov = stats.mov,
            }
            placed[#placed + 1] = pos
            id_counter = id_counter + 1
        else
            break -- Can't find more valid positions
        end
    end
    
    return enemies
end

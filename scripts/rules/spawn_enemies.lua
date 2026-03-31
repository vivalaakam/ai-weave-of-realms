--- Enemy spawn rules.
--
-- Determines enemy spawn positions and stats based on map layout.
-- Enemies should spawn away from the player start (city entrance or road),
-- on passable terrain that is not a POI.
--
-- @param map  Table: map.chunks_wide, map.chunks_tall, map.tiles (flat array)
-- @return     Array of enemy descriptors: { id, x, y, hp, atk, def, spd, mov }
--             Empty table if no valid spawns found.

-- Enemy type definitions
local ENEMY_TYPES = {
    grunt = { hp = 30,  atk = 10, def = 5,  spd = 5,  mov = 3 },
    warrior = { hp = 50, atk = 15, def = 8,  spd = 6,  mov = 3 },
    ranger = { hp = 35,  atk = 18, def = 4,  spd = 8,  mov = 4 },
    boss = { hp = 100,  atk = 25, def = 12, spd = 4,  mov = 2 },
}

-- Tiles enemies can spawn on (excludes POIs)
local SPAWNABLE = {
    meadow = true, forest = true, road = true, bridge = true,
}

-- Tiles to avoid (too close to player start)
local POI_TILES = {
    city = true, city_entrance = true, village = true, merchant = true,
    ruins = true, gold = true, resource = true,
}

local function count_tiles(map, tile_set)
    local count = 0
    for _, kind in ipairs(map.tiles) do
        if tile_set[kind] then
            count = count + 1
        end
    end
    return count
end

local function tile_at(map, x, y)
    local w = map.chunks_wide * 32
    local idx = y * w + x + 1
    if idx < 1 or idx > #map.tiles then
        return nil
    end
    return map.tiles[idx]
end

local function find_player_start(map)
    -- Find city_entrance first, then road, then any passable
    for idx, kind in ipairs(map.tiles) do
        if kind == "city_entrance" then
            local x = (idx - 1) % (map.chunks_wide * 32)
            local y = math.floor((idx - 1) / (map.chunks_wide * 32))
            return x, y
        end
    end

    for idx, kind in ipairs(map.tiles) do
        if kind == "road" then
            local x = (idx - 1) % (map.chunks_wide * 32)
            local y = math.floor((idx - 1) / (map.chunks_wide * 32))
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

local function manhattan(x1, y1, x2, y2)
    return math.abs(x1 - x2) + math.abs(y1 - y2)
end

local function is_valid_spawn(tx, ty, player_x, player_y, placed, min_dist, map_w, map_h)
    -- Check bounds
    if tx < 0 or tx >= map_w or ty < 0 or ty >= map_h then
        return false
    end

    -- Check tile type
    local tile = tile_at({ tiles = map.tiles, chunks_wide = map.chunks_wide }, tx, ty)
    if not SPAWNABLE[tile] then
        return false
    end

    -- Check distance from player start
    if player_x and distance(tx, ty, player_x, player_y) < min_dist then
        return false
    end

    -- Check distance from other placed enemies
    for _, p in ipairs(placed) do
        if distance(tx, ty, p.x, p.y) < 6 then
            return false
        end
    end

    -- Avoid tiles too close to POIs
    for dy = -3, 3 do
        for dx = -3, 3 do
            local nx, ny = tx + dx, ty + dy
            if nx >= 0 and nx < map_w and ny >= 0 and ny < map_h then
                local nt = tile_at({ tiles = map.tiles, chunks_wide = map.chunks_wide }, nx, ny)
                if POI_TILES[nt] then
                    return false
                end
            end
        end
    end

    return true
end

return function(map)
    local player_x, player_y = find_player_start(map)
    local map_w = map.chunks_wide * 32
    local map_h = map.chunks_tall * 32
    local enemies = {}
    local placed = {}

    -- Count spawnable tiles to determine enemy count
    local spawnable_count = count_tiles(map, SPAWNABLE)
    local enemy_count = 0

    if spawnable_count > 200 then
        enemy_count = math.random(4, 7)
    elseif spawnable_count > 100 then
        enemy_count = math.random(2, 4)
    elseif spawnable_count > 50 then
        enemy_count = math.random(1, 2)
    end

    -- Determine minimum distance from player based on map size
    local min_dist = math.max(8, math.min(map_w, map_h) / 6)

    -- Try to place each enemy
    for i = 1, enemy_count do
        local attempts = 0
        local placed = false

        while attempts < 200 and not placed do
            -- Random position
            local tx = math.random(0, map_w - 1)
            local ty = math.random(0, map_h - 1)

            if is_valid_spawn(tx, ty, player_x, player_y, placed, min_dist, map_w, map_h) then
                -- Choose enemy type based on distance from player (further = stronger)
                local dist = player_x and distance(tx, ty, player_x, player_y) or min_dist
                local enemy_type

                if dist > min_dist * 2.5 then
                    -- Far from player: mix of rangers and warriors
                    local roll = math.random()
                    if roll < 0.2 then
                        enemy_type = "boss"
                    elseif roll < 0.6 then
                        enemy_type = "warrior"
                    else
                        enemy_type = "ranger"
                    end
                elseif dist > min_dist * 1.5 then
                    -- Medium distance: warriors and grunts
                    if math.random() < 0.4 then
                        enemy_type = "warrior"
                    else
                        enemy_type = "grunt"
                    end
                else
                    -- Close to min_dist: mostly grunts
                    enemy_type = "grunt"
                end

                local stats = ENEMY_TYPES[enemy_type]
                enemies[#enemies + 1] = {
                    id = i,
                    x = tx,
                    y = ty,
                    hp = stats.hp,
                    atk = stats.atk,
                    def = stats.def,
                    spd = stats.spd,
                    mov = stats.mov,
                }
                placed[#placed + 1] = { x = tx, y = ty }
                placed = true
            end

            attempts = attempts + 1
        end
    end

    return enemies
end

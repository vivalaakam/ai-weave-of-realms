--- City chunk generator (full terrain + city).
--
-- Self-contained generator that produces a complete chunk with:
--   water bodies → rivers → forest clusters → mountain ridges →
--   roads → city block → resource deposits.
--
-- ## City placement
-- - City block: 3×3 tile area at random position (bx, by).
--   - 8 "city" tiles fill rows by..by+1 (all three columns) and
--     the right two cells of row by+2.
--   - "city_entrance" at (bx, by+2) — south isometric tip.
-- - Neighbours of the entrance outside the 3×3 block are forced to
--   "meadow" or "road" (entrance must be accessible).
-- - City is placed BEFORE resources so that the resource stage
--   automatically respects the settlement exclusion zone.
--
-- ## Pipeline support
-- Accepts an optional 4th argument `tiles`.  When provided those tiles
-- are used as the base canvas (skipping all terrain stages); only the
-- city and resource stages run on top.
--
-- @param rng   SeededRng userdata.
-- @param x     Chunk column (0-based).
-- @param y     Chunk row (0-based).
-- @param tiles Optional base tile table from the previous pipeline stage.
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

local function dist2d(x1, y1, x2, y2)
    local dx = x1 - x2
    local dy = y1 - y2
    return math.sqrt(dx * dx + dy * dy)
end

local function place(result, tx, ty, kind, protected)
    if tx < 0 or tx >= CHUNK_SIZE or ty < 0 or ty >= CHUNK_SIZE then return end
    local i = idx(tx, ty)
    if not protected[result[i]] then result[i] = kind end
end

-- ─── Terrain stages (identical to terrain.lua) ────────────────────────────────

local function stage_water(result, rng)
    local PROT = { city=true, city_entrance=true, mountain=true, river=true, road=true }
    local num_lakes = rng:random_range_u32(1, 4)
    for _ = 1, num_lakes do
        local cx = rng:random_range_u32(5, CHUNK_SIZE - 6)
        local cy = rng:random_range_u32(5, CHUNK_SIZE - 6)
        local radius = rng:random_range_u32(3, 7)
        for ty = 0, CHUNK_SIZE - 1 do
            for tx = 0, CHUNK_SIZE - 1 do
                local d = dist2d(tx, ty, cx, cy)
                if d <= radius and rng:random_bool(1.0 - (d / radius) * 0.7) then
                    place(result, tx, ty, "water", PROT)
                end
            end
        end
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
                            place(result, tx + wx, ty + wy, "water", PROT)
                        end
                    end
                end
            end
        end
    end
end

local function stage_rivers(result, rng)
    local PROT = { city=true, city_entrance=true, mountain=true, water=true, road=true }
    local function ep(edge)
        local pos = rng:random_range_u32(3, CHUNK_SIZE - 4)
        if edge == 0 then return pos, 0
        elseif edge == 1 then return CHUNK_SIZE-1, pos
        elseif edge == 2 then return pos, CHUNK_SIZE-1
        else return 0, pos end
    end
    local num_rivers = rng:random_range_u32(1, 3)
    for _ = 1, num_rivers do
        local se = rng:random_range_u32(0, 3)
        local ee = rng:random_bool(0.7) and ((se+2)%4) or ((se+rng:random_range_u32(1,3))%4)
        local cx, cy = ep(se)
        local ex, ey = ep(ee)
        for _ = 1, CHUNK_SIZE * 5 do
            place(result, cx, cy, "river", PROT)
            if rng:random_bool(0.25) then
                local dx = ex-cx; local dy = ey-cy
                local len = math.sqrt(dx*dx+dy*dy)
                if len > 0.5 then
                    place(result, cx+math.floor(-dy/len+0.5), cy+math.floor(dx/len+0.5), "river", PROT)
                end
            end
            if (ee==0 and cy<=0) or (ee==2 and cy>=CHUNK_SIZE-1)
            or (ee==1 and cx>=CHUNK_SIZE-1) or (ee==3 and cx<=0) then break end
            local dx = ex-cx; local dy = ey-cy
            if math.abs(dx)+math.abs(dy) == 0 then break end
            local mx, my
            if rng:random_bool(0.65) then
                if math.abs(dx)>=math.abs(dy) then mx,my=(dx>0 and 1 or -1),0
                else mx,my=0,(dy>0 and 1 or -1) end
            else
                if math.abs(dx)>=math.abs(dy) then mx,my=0,(rng:random_bool(0.5) and 1 or -1)
                else mx,my=(rng:random_bool(0.5) and 1 or -1),0 end
            end
            local nx=clamp(cx+mx,0,CHUNK_SIZE-1); local ny=clamp(cy+my,0,CHUNK_SIZE-1)
            if result[idx(nx,ny)]=="mountain" then
                if mx~=0 then mx,my=0,(dy>0 and 1 or -1) else mx,my=(dx>0 and 1 or -1),0 end
                nx=clamp(cx+mx,0,CHUNK_SIZE-1); ny=clamp(cy+my,0,CHUNK_SIZE-1)
            end
            cx,cy=nx,ny
        end
    end
end

local function stage_forest(result, rng)
    local PROT = { city=true, city_entrance=true, water=true, mountain=true, river=true }
    local num_clusters = rng:random_range_u32(3, 8)
    for _ = 1, num_clusters do
        local cx = rng:random_range_u32(0, CHUNK_SIZE-1)
        local cy = rng:random_range_u32(0, CHUNK_SIZE-1)
        local radius = rng:random_range_u32(3, 8)
        for ty = 0, CHUNK_SIZE-1 do
            for tx = 0, CHUNK_SIZE-1 do
                local d = dist2d(tx, ty, cx, cy)
                if d <= radius and result[idx(tx,ty)]=="meadow" then
                    if rng:random_bool(1.0-(d/radius)*0.8) then
                        place(result, tx, ty, "forest", PROT)
                    end
                end
            end
        end
    end
end

local function stage_mountains(result, rng)
    local OVR  = { meadow=true, forest=true }
    local PROT = { city=true, city_entrance=true, water=true, river=true }
    local function paint_seg(x1,y1,x2,y2,hw)
        local sdx=x2-x1; local sdy=y2-y1
        local slen=math.sqrt(sdx*sdx+sdy*sdy)
        if slen<0.001 then return end
        local mn_x=math.max(0,math.floor(math.min(x1,x2))-hw-1)
        local mx_x=math.min(CHUNK_SIZE-1,math.ceil(math.max(x1,x2))+hw+1)
        local mn_y=math.max(0,math.floor(math.min(y1,y2))-hw-1)
        local mx_y=math.min(CHUNK_SIZE-1,math.ceil(math.max(y1,y2))+hw+1)
        for ty=mn_y,mx_y do for tx=mn_x,mx_x do
            local px=tx-x1; local py=ty-y1
            local t=clamp((px*sdx+py*sdy)/(slen*slen),0.0,1.0)
            local qx=x1+t*sdx-tx; local qy=y1+t*sdy-ty
            local d=math.sqrt(qx*qx+qy*qy)
            if d<=hw then
                local i=idx(tx,ty)
                if OVR[result[i]] and not PROT[result[i]] then
                    if rng:random_bool(1.0-(d/hw)*0.5) then result[i]="mountain" end
                end
            end
        end end
    end
    local num_ridges = rng:random_range_u32(1, 4)
    for _ = 1, num_ridges do
        local num_pts = rng:random_range_u32(3, 6)
        local hw      = rng:random_range_u32(1, 4)
        local px, py  = {}, {}
        px[1]=rng:random_range_u32(0,CHUNK_SIZE-1); py[1]=rng:random_range_u32(0,CHUNK_SIZE-1)
        for i=2,num_pts do
            local step=rng:random_range_u32(8,17); local angle=rng:next_f64()*2.0*math.pi
            px[i]=clamp(px[i-1]+math.floor(step*math.cos(angle)+0.5),0,CHUNK_SIZE-1)
            py[i]=clamp(py[i-1]+math.floor(step*math.sin(angle)+0.5),0,CHUNK_SIZE-1)
        end
        for i=1,num_pts-1 do paint_seg(px[i],py[i],px[i+1],py[i+1],hw) end
    end
end

local function stage_roads(result, rng)
    local BLOCKED = { city=true, city_entrance=true, water=true, mountain=true, river=true }
    local function ep(edge)
        local lo=math.floor(CHUNK_SIZE/3); local hi=math.floor(2*CHUNK_SIZE/3)
        local pos=rng:random_range_u32(lo,hi)
        if edge==0 then return pos,0 elseif edge==1 then return CHUNK_SIZE-1,pos
        elseif edge==2 then return pos,CHUNK_SIZE-1 else return 0,pos end
    end
    local roll=rng:next_f64()
    local num_roads = roll<0.30 and 0 or (roll<0.75 and 1 or 2)
    for _ = 1, num_roads do
        local se=rng:random_range_u32(0,3)
        local ee=rng:random_bool(0.7) and ((se+2)%4) or ((se+rng:random_range_u32(1,3))%4)
        local cx,cy=ep(se); local ex,ey=ep(ee)
        for _ = 1, CHUNK_SIZE*4 do
            if not BLOCKED[result[idx(cx,cy)]] then result[idx(cx,cy)]="road" end
            if (ee==0 and cy<=0) or (ee==2 and cy>=CHUNK_SIZE-1)
            or (ee==1 and cx>=CHUNK_SIZE-1) or (ee==3 and cx<=0) then break end
            local dx=ex-cx; local dy=ey-cy
            if math.abs(dx)+math.abs(dy)==0 then break end
            local mx,my
            if rng:random_bool(0.85) then
                if math.abs(dx)>=math.abs(dy) then mx,my=(dx>0 and 1 or -1),0
                else mx,my=0,(dy>0 and 1 or -1) end
            else
                if math.abs(dx)>=math.abs(dy) then
                    mx,my=0,(dy~=0 and (dy>0 and 1 or -1) or (rng:random_bool(0.5) and 1 or -1))
                else
                    mx,my=(dx~=0 and (dx>0 and 1 or -1) or (rng:random_bool(0.5) and 1 or -1)),0
                end
            end
            local alts={{mx,my},{my,mx},{-my,-mx},{-mx,-my}}
            for _,mv in ipairs(alts) do
                local nx=clamp(cx+mv[1],0,CHUNK_SIZE-1); local ny=clamp(cy+mv[2],0,CHUNK_SIZE-1)
                if not BLOCKED[result[idx(nx,ny)]] then cx,cy=nx,ny; break end
            end
        end
    end
end

-- ─── Stage: city block ────────────────────────────────────────────────────────

local function stage_city(result, rng)
    -- bx ∈ [1,28], by ∈ [1,26] — 3×3 block fits with 1-tile margin on all sides
    local bx = rng:random_range_u32(1, 28)
    local by = rng:random_range_u32(1, 26)

    -- Top two rows: all city
    for dy = 0, 1 do
        for dx = 0, 2 do
            result[idx(bx+dx, by+dy)] = "city"
        end
    end
    -- Bottom row: entrance at left, city on right two
    result[idx(bx,   by+2)] = "city_entrance"
    result[idx(bx+1, by+2)] = "city"
    result[idx(bx+2, by+2)] = "city"

    -- Force entrance neighbours (outside 3×3 block) to meadow or road
    local nb = {
        {bx-1, by+2}, {bx+3, by+2},
        {bx,   by+3}, {bx+1, by+3}, {bx+2, by+3},
    }
    for _, p in ipairs(nb) do
        local nx, ny = p[1], p[2]
        if nx >= 0 and nx < CHUNK_SIZE and ny >= 0 and ny < CHUNK_SIZE then
            result[idx(nx, ny)] = rng:random_bool(0.7) and "meadow" or "road"
        end
    end
end

-- ─── Stage: resources ─────────────────────────────────────────────────────────

local function stage_resources(result, rng)
    local PLACEABLE  = { meadow=true }
    local SETTLEMENT = { city=true, city_entrance=true, village=true }
    local MIN_SPACING = 4
    local SAFE_RADIUS = 2
    local MAX_TRIES   = 60

    local function near_settlement(tx, ty)
        for dy=-SAFE_RADIUS,SAFE_RADIUS do for dx=-SAFE_RADIUS,SAFE_RADIUS do
            local nx=tx+dx; local ny=ty+dy
            if nx>=0 and nx<CHUNK_SIZE and ny>=0 and ny<CHUNK_SIZE then
                if SETTLEMENT[result[idx(nx,ny)]] then return true end
            end
        end end
        return false
    end

    local placed = {}
    local function spaced_ok(tx, ty)
        for _,p in ipairs(placed) do
            if dist2d(tx,ty,p[1],p[2]) < MIN_SPACING then return false end
        end
        return true
    end

    local function try_place(kind)
        for _ = 1, MAX_TRIES do
            local tx=rng:random_range_u32(1,CHUNK_SIZE-2)
            local ty=rng:random_range_u32(1,CHUNK_SIZE-2)
            if PLACEABLE[result[idx(tx,ty)]] and not near_settlement(tx,ty) and spaced_ok(tx,ty) then
                result[idx(tx,ty)] = kind
                placed[#placed+1] = {tx,ty}
                return true
            end
        end
        return false
    end

    try_place("gold")
    local num_res = rng:random_range_u32(4, 8)
    for _ = 1, num_res do try_place("resource") end
end

-- ─── Entry point ──────────────────────────────────────────────────────────────

local function generate_chunk(rng, x, y, tiles)
    local result = {}

    if tiles ~= nil then
        -- Pipeline mode: use base tiles, only apply city + resources on top
        for i = 1, N do result[i] = tiles[i] end
    else
        -- Standalone mode: run full terrain pipeline first
        for i = 1, N do result[i] = "meadow" end
        stage_water(result, rng)
        stage_rivers(result, rng)
        stage_forest(result, rng)
        stage_mountains(result, rng)
        stage_roads(result, rng)
    end

    stage_city(result, rng)
    stage_resources(result, rng)

    return result
end

return generate_chunk

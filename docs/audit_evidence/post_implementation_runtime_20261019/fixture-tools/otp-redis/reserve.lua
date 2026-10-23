
local limits = {30, 1, 3, 20}
local windows = {60, 60, 900, 86400}
for i = 1, 4 do
    local value = redis.call('GET', KEYS[i])
    local count = 0
    if value then count = tonumber(value) end
    if not count or count < 0 or count ~= math.floor(count) then
        return redis.error_reply('Invalid delivery budget counter')
    end
    if count >= limits[i] then return 0 end
end
for i = 1, 4 do
    local count = redis.call('INCR', KEYS[i])
    if count == 1 then redis.call('EXPIRE', KEYS[i], windows[i]) end
end
return 1

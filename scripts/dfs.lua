-- DFS krokowy z użyciem coroutines
local visited = {}
local stack = {}

-- startowy węzeł
local start_node = graph:get_nodes()[1]
table.insert(stack, start_node)

while #stack > 0 do
    local node = table.remove(stack) -- bierzemy ostatni element (DFS LIFO)

    if not visited[node] then
        visited[node] = true
        set_color(node, "#0f0")   -- odwiedzony węzeł na zielono
        coroutine.yield()

        local neighbours = graph:get_neighbours(node)
        for i = #neighbours, 1, -1 do -- odwrócone, żeby DFS wygląda naturalnie
            local n = neighbours[i]
            if not visited[n] then
                table.insert(stack, n)
            end
        end
    end

    -- opcjonalnie reset koloru po przetworzeniu węzła
    reset_color(node)
    coroutine.yield()
end
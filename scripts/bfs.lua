
                local visited = {}
                local queue = {}

                -- dodajemy pierwszy węzeł do kolejki
                start_node = graph:get_nodes()[1]
                table.insert(queue, start_node)
                visited[start_node] = true
                set_color(start_node, [[#0f0]])  -- startowy węzeł na zielono
                coroutine.yield()

                while #queue > 0 do
                    local node = table.remove(queue, 1) -- zdejmujemy pierwszy w kolejce

                    local neighbours = graph:get_neighbours(node)
                    for _, n in ipairs(neighbours) do
                        if not visited[n] then
                            table.insert(queue, n)
                            visited[n] = true
                            set_color(n, [[#f00]])  -- odkryty sąsiad na czerwono
                            coroutine.yield()
                        end
                    end

                    -- po odwiedzeniu wszystkich sąsiadów możemy zresetować kolor węzła
                    reset_color(node)
                    coroutine.yield()
                end
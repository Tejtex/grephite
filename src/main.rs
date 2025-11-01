use bevy::a11y::ManageAccessibilityUpdates;
use bevy::window::PrimaryWindow;
use bevy::{color::palettes::css::*, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use bevy_pancam::{PanCam, PanCamPlugin};
use rand::prelude::*;
use std::collections::HashMap;
use std::fs;

pub mod components;
pub mod physics;
mod scripts;

use crate::components::*;
use crate::scripts::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Grephite".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PanCamPlugin::default())
        .add_plugins(EguiPlugin::default())
        .insert_state(AppMode::View)
        .insert_resource(DragState::default())
        .insert_resource(Config {
            k_r: 5000.,
            k_g: 0.2,
            enabled: true,
            scripts_dir: "scripts".to_string(),
        })
        .insert_resource(Selected(None))
        .insert_resource(Graph::default())
        .insert_resource(EdgeCreation::default())
        .insert_resource(DeletionRequest::default())
        .insert_resource(NodeColors {
            colors: HashMap::new(),
        })
        .insert_resource(LuaManager::default())
        .add_message::<ScriptCommand>()
        .add_message::<StepLua>()
        .add_message::<ExecLuaScript>()
        .add_systems(Startup, (load_edge_list,).chain())
        .add_systems(
            Update,
            (
                draw_edges,
                crate::physics::apply_forces,
                pan_camera_system,
                drag_nodes,
                draw_nodes,
                create_node.run_if(in_state(AppMode::Edit)),
                create_edge.run_if(in_state(AppMode::Edit)),
                draw_edge_preview.run_if(in_state(AppMode::Edit)),
                detect_right_clicks.run_if(in_state(AppMode::Edit)),
                spawn_lua_scripts,
                run_lua_scripts,
                flush_lua_events,
                exec_lua_events,
                auto_run,
            ),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                ui_system,
                deletion_popup.run_if(in_state(AppMode::Edit)),
                script_ui.run_if(in_state(AppMode::Script)),
            ),
        )
        .run();
}

fn compute_degrees(edge_query: &Query<&GEdge>) -> HashMap<Entity, usize> {
    let mut degrees = HashMap::new();

    for edge in edge_query.iter() {
        *degrees.entry(edge.from).or_insert(0) += 1;
        *degrees.entry(edge.to).or_insert(0) += 1;
    }

    degrees
}

fn drag_nodes(
    mut drag: ResMut<DragState>,
    mut camera: Query<(&Camera, &GlobalTransform, &mut PanCam)>,
    mut nodes: Query<(Entity, &mut Transform, &GNode)>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut world_pos: Local<Vec2>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut selected: ResMut<Selected>,
    egui_ctx: EguiContexts,
) -> Result {
    if egui_ctx.ctx()?.wants_pointer_input() {
        return Ok(());
    }
    let mut camera = camera.single_mut().unwrap();
    let window = window.single().unwrap();
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.0.viewport_to_world(camera.1, cursor).ok())
        .map(|ray| ray.origin.truncate())
    {
        *world_pos = world_position;
    }

    // start drag
    if mouse_input.just_pressed(MouseButton::Left) && drag.dragging.is_none() {
        let mut on = false;
        for (ent, tf, _) in nodes.iter() {
            let dist = (tf.translation.truncate() - *world_pos).length();
            if dist < 60.0 {
                // node radius
                on = true;
                drag.dragging = Some(ent);
                camera.2.enabled = false;
                drag.offset = tf.translation.truncate() - *world_pos;
                if selected.0 == None {
                    selected.0 = Some(ent);
                } else {
                    selected.0 = None;
                }
                break;
            }
        }
        if !on {
            drag.dragging = None;
            selected.0 = None;
        }
    }

    // update drag
    if let Some(ent) = drag.dragging {
        if let Ok((_, mut tf, _)) = nodes.get_mut(ent) {
            tf.translation = (*world_pos + drag.offset).extend(tf.translation.z);
        }
    }

    // release drag
    if mouse_input.just_released(MouseButton::Left) {
        camera.2.enabled = true;
        drag.dragging = None;
    }
    Ok(())
}

fn create_node(
    mut camera: Query<(&Camera, &GlobalTransform)>,
    nodes: Query<(Entity, &Transform, &GNode)>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut graph: ResMut<Graph>,
    egui_ctx: EguiContexts,
    mut world_pos: Local<Vec2>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut colors: ResMut<NodeColors>,
) -> Result {
    if egui_ctx.ctx()?.wants_pointer_input() {
        return Ok(());
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return Ok(());
    }
    let camera = camera.single_mut().unwrap();
    let window = window.single().unwrap();
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.0.viewport_to_world(camera.1, cursor).ok())
        .map(|ray| ray.origin.truncate())
    {
        *world_pos = world_position;
    }
    let mut on = false;
    for (ent, tf, _) in nodes.iter() {
        let dist = (tf.translation.truncate() - *world_pos).length();
        if dist < 60.0 {
            on = true;
        }
    }
    if on {
        return Ok(());
    }

    let font = asset_server.load("fonts/FiraMono-Regular.ttf");
    let text_font = TextFont {
        font: font.clone(),
        font_size: 50.0,
        ..default()
    };
    graph.curr_id += 1;
    let id = commands
        .spawn((
            GNode { id: graph.curr_id },
            Mesh2d(meshes.add(Circle::new(50.))),
            MeshMaterial2d(materials.add(Color::from(BLACK))),
            Transform::from_translation(Vec3::new(world_pos.x, world_pos.y, 1.0)),
            GlobalTransform::default(),
            Text2d::new(graph.curr_id.to_string()),
            text_font.clone(),
        ))
        .id();
    graph.adj.insert(id, Vec::new());
    colors.colors.insert(id, Color::from(BLACK));
    Ok(())
}

fn create_edge(
    mut commands: Commands,
    mut edge_state: ResMut<EdgeCreation>,
    nodes: Query<(Entity, &Transform), With<GNode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut graph: ResMut<Graph>,
) -> Result {
    if !mouse.just_pressed(MouseButton::Left) {
        return Ok(());
    }

    let (camera, camera_tf) = camera.single()?;
    let Some(cursor) = window.single()?.cursor_position() else {
        return Ok(());
    };
    let Some(world_pos) = camera
        .viewport_to_world(camera_tf, cursor)
        .ok()
        .map(|ray| ray.origin.truncate())
    else {
        return Ok(());
    };

    // Did we click on a node?
    let clicked_node = nodes
        .iter()
        .find(|(_, tf)| (tf.translation.truncate() - world_pos).length() < 60.0)
        .map(|(e, _)| e);

    if let Some(node) = clicked_node {
        match edge_state.from {
            None => {
                // first click — start new edge
                edge_state.from = Some(node);
            }
            Some(from) if from != node => {
                // second click — finalize edge
                let edge = commands
                    .spawn((
                        GEdge { from, to: node },
                        Mesh2d(meshes.add(Rectangle::new(0., 0.))),
                        MeshMaterial2d(materials.add(ColorMaterial::from(Color::from(RED)))),
                        Transform::default(),
                        GlobalTransform::default(),
                    ))
                    .id();
                graph.adj.entry(from).or_insert(Vec::new()).push(node);
                graph.adj.entry(node).or_insert(Vec::new()).push(from);
                commands.entity(edge_state.temp_line.unwrap()).despawn();
                edge_state.from = None;
                edge_state.temp_line = None;
            }
            _ => {
                // clicked the same node again → cancel
                edge_state.from = None;
            }
        }
    } else {
        // clicked empty space — cancel
        edge_state.from = None;
    }
    Ok(())
}

fn draw_edge_preview(
    mut edge_state: ResMut<EdgeCreation>,
    mut commands: Commands,
    nodes: Query<&Transform, With<GNode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut temp_query: Query<(&mut Transform, &mut Mesh2d), Without<GNode>>,
) -> Result {
    if let Some(from) = edge_state.from {
        let (camera, camera_tf) = camera.single()?;
        let Some(cursor) = window.single()?.cursor_position() else {
            return Ok(());
        };
        let Some(world_pos) = camera
            .viewport_to_world(camera_tf, cursor)
            .ok()
            .map(|ray| ray.origin.truncate())
        else {
            return Ok(());
        };

        let from_pos = nodes.get(from).ok().map(|t| t.translation.truncate());
        if let Some(start) = from_pos {
            let direction = world_pos - start;
            let length = direction.length();
            let angle = direction.y.atan2(direction.x);

            // If temp line doesn’t exist, create it
            let temp_line = edge_state.temp_line.get_or_insert_with(|| {
                commands
                    .spawn((
                        Mesh2d(meshes.add(Rectangle::new(length, 2.))),
                        MeshMaterial2d(materials.add(ColorMaterial::from(Color::from(GRAY)))),
                        Transform::from_translation(Vec3::new(
                            (start.x + world_pos.x) / 2.0,
                            (start.y + world_pos.y) / 2.0,
                            0.0,
                        ))
                        .with_rotation(Quat::from_rotation_z(angle)),
                        GlobalTransform::default(),
                    ))
                    .id()
            });

            // Update existing line
            if let Ok((mut tf, mut mesh2d)) = temp_query.get_mut(*temp_line) {
                tf.translation = Vec3::new(
                    (start.x + world_pos.x) / 2.0,
                    (start.y + world_pos.y) / 2.0,
                    0.0,
                );
                tf.rotation = Quat::from_rotation_z(angle);
                *mesh2d = Mesh2d(meshes.add(Rectangle::new(length, 2.)));
            }
        }
    } else if let Some(temp_line) = edge_state.temp_line.take() {
        // Remove preview line if cancelled
        commands.entity(temp_line).despawn();
    }
    Ok(())
}

fn load_edge_list(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut graph: ResMut<Graph>,
    mut colors: ResMut<NodeColors>,
) -> Result {
    let content = fs::read_to_string("graph.edges").expect("Unable to read file");
    let mut node_map: HashMap<usize, Entity> = HashMap::new();
    let font = asset_server.load("fonts/FiraMono-Regular.ttf");
    let mut rng = rand::rng();
    let text_font = TextFont {
        font: font.clone(),
        font_size: 50.0,
        ..default()
    };

    // Camera
    commands.spawn((Camera2d::default(), PanCam::default()));

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let from_id = parts[0].to_string().parse()?;
        let to_id = parts[1].to_string().parse()?;
        graph.curr_id = graph.curr_id.max(from_id).max(to_id);

        let from_ent = *node_map.entry(from_id).or_insert_with(|| {
            let ent = commands
                .spawn((
                    GNode { id: from_id },
                    Mesh2d(meshes.add(Circle::new(50.))),
                    MeshMaterial2d(materials.add(Color::from(BLACK))),
                    Transform::from_translation(Vec3::new(
                        rng.random_range(-50f32..50f32),
                        rng.random_range(-50f32..50f32),
                        1.0,
                    )),
                    GlobalTransform::default(),
                    Text2d::new(from_id.to_string()),
                    text_font.clone(),
                ))
                .id();

            ent
        });

        let to_ent = *node_map.entry(to_id).or_insert_with(|| {
            let ent = commands
                .spawn((
                    Text2d::new(to_id.to_string()),
                    text_font.clone(),
                    GNode { id: to_id },
                    Mesh2d(meshes.add(Circle::new(50.))),
                    MeshMaterial2d(materials.add(Color::from(BLACK))),
                    Transform::from_translation(Vec3::new(
                        rng.random_range(-50f32..50f32),
                        rng.random_range(-50f32..50f32),
                        1.0,
                    )),
                    GlobalTransform::default(),
                ))
                .id();
            ent
        });
        graph.adj.entry(from_ent).or_insert(Vec::new()).push(to_ent);
        graph.adj.entry(to_ent).or_insert(Vec::new()).push(from_ent);
        let ent = commands
            .spawn((
                GEdge {
                    from: from_ent,
                    to: to_ent,
                },
                Mesh2d(meshes.add(Rectangle::new(0., 0.))),
                MeshMaterial2d(materials.add(ColorMaterial::from(Color::from(RED)))),
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        graph.edges.push(ent);
        colors.colors.insert(to_ent, Color::from(BLACK));
        colors.colors.insert(from_ent, Color::from(BLACK));
    }
    Ok(())
}

fn draw_edges(
    query_nodes: Query<(&Transform, &GNode), Without<GEdge>>,
    mut query_edges: Query<(&mut Transform, &mut Mesh2d, &GEdge)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (mut transform, mut mesh2d, edge) in query_edges.iter_mut() {
        if let (Ok((from_tf, _)), Ok((to_tf, _))) =
            (query_nodes.get(edge.from), query_nodes.get(edge.to))
        {
            let start = from_tf.translation.truncate();
            let end = to_tf.translation.truncate();
            let direction = end - start;
            let length = direction.length();
            let angle = direction.y.atan2(direction.x);

            let mesh = meshes.add(Mesh::from(Rectangle::new(length, 2.)));
            *mesh2d = Mesh2d(mesh);
            *transform = Transform {
                translation: Vec3::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0, 0.0),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            };
        }
    }
}

fn draw_nodes(
    mut query_nodes: Query<(Entity, &mut MeshMaterial2d<ColorMaterial>), With<GNode>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    selected: Res<Selected>,
    colors: Res<NodeColors>,
) {
    // Reset all nodes to BLACK
    for (entity, mut mat) in query_nodes.iter_mut() {
        if Some(entity) == selected.0 {
            *mat = MeshMaterial2d(materials.add(Color::from(RED)));
        } else {
            *mat = MeshMaterial2d(
                materials.add(*colors.colors.get(&entity).unwrap_or(&Color::from(BLACK))),
            );
        }
    }
}

fn detect_right_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    nodes: Query<(Entity, &Transform), With<GNode>>,
    edges: Query<(Entity, &GEdge, &Transform)>,
    mut deletion: ResMut<DeletionRequest>,
) -> Result {
    if !mouse.just_pressed(MouseButton::Right) {
        return Ok(());
    }

    let (camera, camera_tf) = camera.single()?;
    let Some(cursor) = window.single()?.cursor_position() else {
        return Ok(());
    };
    let Some(world_pos) = camera
        .viewport_to_world(camera_tf, cursor)
        .ok()
        .map(|ray| ray.origin.truncate())
    else {
        return Ok(());
    };

    // 1️⃣ Check nodes first
    for (ent, tf) in nodes.iter() {
        if (tf.translation.truncate() - world_pos).length() < 60.0 {
            deletion.node = Some(ent);
            return Ok(()); // found a node
        }
    }

    // 2️⃣ Otherwise check edges (approximate with distance to line)
    for (ent, edge, _) in edges.iter() {
        if let (Ok(from_tf), Ok(to_tf)) = (nodes.get(edge.from), nodes.get(edge.to)) {
            let a = from_tf.1.translation.truncate();
            let b = to_tf.1.translation.truncate();
            if point_near_segment(a, b, world_pos, 5.0) {
                deletion.edge = Some(ent);
                return Ok(());
            }
        }
    }

    Ok(())
}
fn point_near_segment(a: Vec2, b: Vec2, p: Vec2, tolerance: f32) -> bool {
    let ab = b - a;
    let ap = p - a;
    let t = (ap.dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    let closest = a + t * ab;
    p.distance(closest) < tolerance
}
fn deletion_popup(
    mut egui_ctx: EguiContexts,
    mut deletion: ResMut<DeletionRequest>,
    mut commands: Commands,
    mut graph: ResMut<Graph>,
    edges: Query<(Entity, &GEdge)>,
) -> Result {
    if let Some(node_ent) = deletion.node {
        egui::Window::new("Delete Node?")
            .collapsible(false)
            .show(egui_ctx.ctx_mut()?, |ui| {
                ui.label("Delete this node and all connected edges?");
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        deletion.node = None;
                    }
                    if ui.button("Delete").clicked() {
                        // remove all edges linked to this node
                        delete_node(deletion.node.unwrap(), &mut graph, &mut commands, &edges);
                        deletion.node = None;
                    }
                });
            });
    }

    if let Some(edge_ent) = deletion.edge {
        egui::Window::new("Delete Edge?")
            .collapsible(false)
            .show(egui_ctx.ctx_mut()?, |ui| {
                ui.label("Delete this edge?");
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        deletion.edge = None;
                    }
                    if ui.button("Delete").clicked() {
                        delete_edge(deletion.edge.unwrap(), &mut graph, &mut commands, &edges);
                        deletion.edge = None;
                    }
                });
            });
    }

    Ok(())
}
fn delete_node(
    node: Entity,
    graph: &mut Graph,
    commands: &mut Commands,
    edge_query: &Query<(Entity, &GEdge)>,
) {
    // 1. Collect all edges connected to this node
    let edges_to_remove: Vec<_> = edge_query
        .iter()
        .filter(|(_, e)| e.from == node || e.to == node)
        .map(|(ent, _)| ent)
        .collect();

    // 2. Despawn those edges
    for edge_ent in &edges_to_remove {
        commands.entity(*edge_ent).despawn();
        graph.edges.retain(|&e| e != *edge_ent);
    }

    // 3. Remove node from adjacency list
    graph.adj.remove(&node);

    // 4. Remove node from other adjacency lists
    for neighbors in graph.adj.values_mut() {
        neighbors.retain(|&n| n != node);
    }

    // 5. Despawn node itself
    commands.entity(node).despawn();
}

fn delete_edge(
    edge_ent: Entity,
    graph: &mut Graph,
    commands: &mut Commands,
    edge_query: &Query<(Entity, &GEdge)>,
) {
    if let Ok((_, edge)) = edge_query.get(edge_ent) {
        // Remove from adjacency list
        if let Some(neighbors) = graph.adj.get_mut(&edge.from) {
            neighbors.retain(|&n| n != edge.to);
        }

        // Remove edge record
        graph.edges.retain(|&e| e != edge_ent);

        // Despawn edge
        commands.entity(edge_ent).despawn();
    }
}

fn ui_system(
    mut egui_ctx: EguiContexts,
    mut config: ResMut<Config>,
    mut next_state: ResMut<NextState<AppMode>>,
    mut writer: MessageWriter<ExecLuaScript>,
    mut writer2: MessageWriter<StepLua>,
    manager: Res<LuaManager>,
) -> Result {
    egui::Window::new("Mode").show(egui_ctx.ctx_mut()?, |ui| {
        if ui.button("View").clicked() {
            next_state.set(AppMode::View);
        }
        if ui.button("Edit").clicked() {
            next_state.set(AppMode::Edit);
        }
        if ui.button("Script").clicked() {
            next_state.set(AppMode::Script);
        }
    });
    egui::Window::new("Physics settings").show(egui_ctx.ctx_mut()?, |ui| {
        ui.checkbox(&mut config.enabled, "Enable physics");
        ui.add(egui::Slider::new(&mut config.k_r, 0.0..=10000.0).text("Repulsion force"));
        ui.add(egui::Slider::new(&mut config.k_g, 0.0..=4.0).text("Gravity force"));
    });

    Ok(())
}

fn script_ui(
    mut egui_ctx: EguiContexts,
    mut next_state: ResMut<NextState<AppMode>>,
    mut writer: MessageWriter<ExecLuaScript>,
    mut writer2: MessageWriter<StepLua>,
    mut manager: ResMut<LuaManager>,
    config: ResMut<Config>,
) -> Result {
    egui::Window::new("Available Scripts").show(egui_ctx.ctx_mut()?, |ui| {
        // 1. wczytujemy wszystkie pliki Lua z katalogu
        if let Ok(entries) = fs::read_dir(&config.scripts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".lua") {
                        if ui.button(name).clicked() {
                            // 2. kliknięcie ładuje skrypt
                            if let Ok(code) = fs::read_to_string(&path) {
                                writer.write(ExecLuaScript { code });
                            }
                        }
                    }
                }
            }
        }

        // 3. Sterowanie skryptem
        if let Some(active) = &mut manager.active_script {
            ui.horizontal(|ui| {
                if ui.button("Step").clicked() {
                    writer2.write(StepLua);
                }

                let start_pause_label = if active.running { "Pause" } else { "Start" };
                if ui.button(start_pause_label).clicked() {
                    active.running = !active.running;
                }

                ui.add(egui::Slider::new(&mut active.speed, 1.0..=100.0).text("Steps/s"));
            });
        };
    });

    Ok(())
}

fn pan_camera_system(
    mut pan: Query<&mut PanCam>,
    drag: Res<DragState>,
    egui_ctx: EguiContexts,
) -> Result {
    let mut pan = pan.single_mut()?;
    if egui_ctx.ctx()?.wants_pointer_input() {
        pan.enabled = false;
        return Ok(());
    }
    if drag.dragging == None {
        pan.enabled = true;
        return Ok(());
    }
    Ok(())
}

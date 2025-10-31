use bevy::window::PrimaryWindow;
use bevy::{color::palettes::css::*, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use bevy_pancam::{PanCam, PanCamPlugin};
use rand::prelude::*;
use std::collections::HashMap;
use std::fs;

pub mod components;
pub mod physics;

use crate::components::*;
use crate::physics::*;

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
            k_w: 0.1,
            k_g: 0.2,
            enabled: true,
        })
        .insert_resource(Selected(None))
        .insert_resource(Graph::default())
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
            ),
        )
        .add_systems(EguiPrimaryContextPass, (ui_system,))
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
    mut nodes: Query<(Entity, &mut Transform, &Node)>,
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
        eprintln!("GI");
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
                    selected.0 = Some(ent)
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
        eprintln!("UM?");
        camera.2.enabled = true;
        drag.dragging = None;
    }
    Ok(())
}

fn create_node(
    mut camera: Query<(&Camera, &GlobalTransform)>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut graph: ResMut<Graph>,
    egui_ctx: EguiContexts,
    mut world_pos: Local<Vec2>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mouse_input: Res<ButtonInput<MouseButton>>,
) -> Result {
    if egui_ctx.ctx()?.wants_pointer_input() {
        return Ok(());
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
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
    Ok(())
}

fn load_edge_list(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut graph: ResMut<Graph>,
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
        let ent = commands
            .spawn((
                GEdge {
                    from: from_ent,
                    to: to_ent,
                    weight: parts.get(2).and_then(|s| s.parse().ok()),
                },
                Mesh2d(meshes.add(Rectangle::new(0., 0.))),
                MeshMaterial2d(materials.add(ColorMaterial::from(Color::from(RED)))),
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        graph.edges.push(ent);
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

            let mesh = meshes.add(Mesh::from(Rectangle::new(
                length,
                edge.weight.unwrap_or(2.),
            )));
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
    mut query_nodes: Query<(Entity, &mut MeshMaterial2d<ColorMaterial>), With<Node>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    selected: Res<Selected>,
) {
    // Reset all nodes to BLACK
    for (entity, mut mat) in query_nodes.iter_mut() {
        if Some(entity) == selected.0 {
            *mat = MeshMaterial2d(materials.add(Color::from(RED)));
        } else {
            *mat = MeshMaterial2d(materials.add(Color::from(BLACK)));
        }
    }
}

fn ui_system(
    mut egui_ctx: EguiContexts,
    mut config: ResMut<Config>,
    mut next_state: ResMut<NextState<AppMode>>,
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
        ui.add(egui::Slider::new(&mut config.k_w, 0.0..=1.0).text("The weight of weights"));
        ui.add(egui::Slider::new(&mut config.k_g, 0.0..=4.0).text("Gravity force"));
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

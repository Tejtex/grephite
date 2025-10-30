use bevy::{color::palettes::css::*, prelude::*};
use bevy_egui::{egui, EguiPlugin};
use rand::prelude::*;
use std::collections::HashMap;
use std::fs;
use bevy_pancam::{PanCamPlugin, PanCam};
use std::ops::AddAssign;

#[derive(Component)]
struct Node {
    id: String,
}

#[derive(Component)]
struct Edge {
    from: Entity,
    to: Entity,
    weight: Option<f32>
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Graphite".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PanCamPlugin::default())
        // .add_plugin(EguiPlugin)
        
        .add_systems(Startup, load_edge_list)
        .add_systems(Update, (draw_edges, apply_forces))
        .run();
}

fn load_edge_list(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let content = fs::read_to_string("graph.edges").expect("Unable to read file");
    let mut node_map: HashMap<String, Entity> = HashMap::new();
    let mut rng = rand::rng();

    // Camera
    commands.spawn((Camera2d::default(), PanCam::default()));

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let from_id = parts[0].to_string();
        let to_id   = parts[1].to_string();

        let from_ent = *node_map.entry(from_id.clone()).or_insert_with(|| {
            let ent = commands.spawn((
                Node { id: from_id.clone() },
                Mesh2d(meshes.add(Circle::new(10.))),
                MeshMaterial2d(materials.add(Color::from(WHITE))),
                Transform::from_translation(Vec3::new(rng.random_range(-500f32..500f32), rng.random_range(-500f32..500f32), 1.0)),
                GlobalTransform::default(),
            )).id();
            ent
        });

        let to_ent = *node_map.entry(to_id.clone()).or_insert_with(|| {
            let ent = commands.spawn((
                Node { id: to_id.clone() },
                Mesh2d(meshes.add(Circle::new(10.))),
                MeshMaterial2d(materials.add(Color::from(WHITE))),
                Transform::from_translation(Vec3::new(rng.random_range(-500f32..500f32), rng.random_range(-500f32..500f32), 1.0)),
                GlobalTransform::default(),
            )).id();
            ent
        });

        commands.spawn((
            Edge { from: from_ent, to: to_ent, weight: parts.get(2).and_then(|s| s.parse().ok()) }, 
            Mesh2d(meshes.add(Rectangle::new(0., 0.))),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::from(RED)))),
            Transform::default(),
            GlobalTransform::default(),
        ));
    }
}

fn draw_edges(
    mut commands: Commands,
    query_nodes: Query<(&Transform, &Node), Without<Edge>>,
    mut query_edges: Query<(&mut Transform, &mut Mesh2d, &Edge)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (mut transform, mut mesh2d, edge) in query_edges.iter_mut() {
        if let (Ok((from_tf, _)), Ok((to_tf, _))) = (
            query_nodes.get(edge.from),
            query_nodes.get(edge.to),
        ) {
            let start = from_tf.translation.truncate();
            let end = to_tf.translation.truncate();
            let direction = end - start;
            let length = direction.length();
            let angle = direction.y.atan2(direction.x);

            let mesh = meshes.add(Mesh::from(Rectangle::new(length, edge.weight.unwrap_or(2.))));
            *mesh2d = Mesh2d(mesh);
            *transform = Transform {
                translation: Vec3::new((start.x + end.x)/2.0, (start.y + end.y)/2.0, 0.0),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            };
            commands.spawn((
            ));
            
        }
    }
}

fn apply_forces(
    mut query: Query<(Entity, &mut Transform, &Node)>,
    edge_query: Query<&Edge>,
) {
    let spring_k = 0.2;
    let ideal_len = 50.0;
    let repulsion_k = 1000.0;
    let timestep = 0.016;

    // collect positions
    let mut positions: HashMap<Entity, Vec2> = HashMap::new();
    for (ent, tf, _) in query.iter() {
        positions.insert(ent, tf.translation.truncate());
    }

    // bucket nodes into coarse grid for repulsion approx
    let bucket_size = ideal_len * 2.0;
    let mut buckets: HashMap<(i32,i32), Vec<Entity>> = HashMap::new();
    for (ent, pos) in positions.iter() {
        let bx = (pos.x / bucket_size).floor() as i32;
        let by = (pos.y / bucket_size).floor() as i32;
        buckets.entry((bx,by)).or_default().push(*ent);
    }

    // initialize forces
    let mut forces: HashMap<Entity, Vec2> = positions.keys().map(|&e| (e, Vec2::ZERO)).collect();

    // repulsion: approximate by only checking near buckets
    for (&ent, &pos) in positions.iter() {
        let bx = (pos.x / bucket_size).floor() as i32;
        let by = (pos.y / bucket_size).floor() as i32;
        for ox in (bx-1)..=(bx+1) {
            for oy in (by-1)..=(by+1) {
                if let Some(bucket_entities) = buckets.get(&(ox,oy)) {
                    for &other in bucket_entities.iter() {
                        if other == ent { continue; }
                        if let Some(&pos2) = positions.get(&other) {
                            let delta = pos - pos2;
                            let dist_sq = delta.length_squared().max(0.01);
                            let force_mag = repulsion_k / dist_sq;
                            let dir = delta.normalize_or_zero();
                            forces.get_mut(&ent).unwrap().add_assign(dir * force_mag);
                        }
                    }
                }
            }
        }
    }

    // spring (attraction) forces
    for edge in edge_query.iter() {
        if let (Some(&pf), Some(&pt)) = (positions.get(&edge.from), positions.get(&edge.to)) {
            let delta = pf - pt;
            let dist = delta.length().max(0.01);
            let dir = delta.normalize_or_zero();
            let stretch = dist - ideal_len;
            let f = -spring_k * stretch * dir;
            *forces.get_mut(&edge.from).unwrap() += f;
            *forces.get_mut(&edge.to).unwrap() -= f;
        }
    }

    // apply forces (Euler integration)
    for (ent, mut tf, _) in query.iter_mut() {
        if let Some(f) = forces.get(&ent) {
            let new_pos2 = tf.translation.truncate() + *f * timestep;
            tf.translation = new_pos2.extend(tf.translation.z);
        }
    }
}


// fn ui_panel(mut egui_ctx: ResMut<bevy_egui::EguiContext>) {
//     egui::Window::new("Controls").show(egui_ctx.ctx_mut(), |ui| {
//         ui.label("Graph loader example");
//         if ui.button("Reload").clicked() {
//             // implement re‑load logic
//         }
//     });
// }

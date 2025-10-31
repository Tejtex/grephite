use std::cmp::Reverse;
use bevy::{color::palettes::css::*, prelude::*};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use rand::prelude::*;
use std::collections::{BinaryHeap, HashMap};
use std::fs;
use bevy_pancam::{PanCamPlugin, PanCam};
use std::ops::AddAssign;
use bevy::audio::Sample;
use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::window::PrimaryWindow;
use ordered_float::OrderedFloat;

#[derive(Component)]
struct Node {
    id: usize
}

#[derive(Component)]
struct Edge {
    from: Entity,
    to: Entity,
    weight: Option<f32>
}
#[derive(Resource, Default)]
struct DragState {
    dragging: Option<Entity>,
    offset: Vec2, // offset from mouse to node center
}
#[derive(Resource)]
struct Config {
    k_r: f32,
    k_w: f32,
    enabled: bool
}

#[derive(Resource)]
#[derive(Debug)]
enum State {
    Normal,
    Dijstra {
        dist: HashMap<Entity, f32>,
        from: Entity
    },
}

#[derive(Message)]
#[derive(PartialEq, Default)]
#[derive(Debug)]
enum AlgoEvent {
    #[default]
    None,
    Dijstra,
    BFS
}

#[derive(Resource)]
struct Selected(Option<Entity>);

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
        .insert_resource(DragState::default())
        .insert_resource(State::Normal)
        .insert_resource(Config {
            k_r: 5000.,
            k_w: 0.1,
            enabled: true
        })
        .insert_resource(Selected(None))
        .add_message::<AlgoEvent>()
        .add_systems(Startup, (load_edge_list,).chain())
        .add_systems(Update, (draw_edges, apply_forces, pan_camera_system, drag_nodes, draw_nodes, dijkstra))
        .add_systems(EguiPrimaryContextPass, (ui_system, ))
        .run();
}

fn compute_degrees(edge_query: &Query<&Edge>) -> HashMap<Entity, usize> {
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
    if let Some(world_position) = window.cursor_position()
        .and_then(|cursor| camera.0.viewport_to_world(camera.1, cursor).ok())
        .map(|ray| ray.origin.truncate())
    {
        *world_pos = world_position;
    }

    // start drag

    if mouse_input.just_pressed(MouseButton::Left) && drag.dragging.is_none()  {
        let mut on = false;
        for (ent, tf, _) in nodes.iter() {
            let dist = (tf.translation.truncate() - *world_pos).length();
            if dist < 60.0 { // node radius
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
        if (!on) {
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
    Ok((
        
        ))
}
fn load_edge_list(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>
) -> Result {
    let content = fs::read_to_string("graph.edges").expect("Unable to read file");
    let mut node_map: HashMap<usize, Entity> = HashMap::new();
    let font = asset_server.load("fonts\\FiraMono-Regular.ttf");
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
        if parts.len() < 2 { continue; }
        let from_id = parts[0].to_string().parse()?;
        let to_id   = parts[1].to_string().parse()?;

        let from_ent = *node_map.entry(from_id).or_insert_with(|| {
            let ent = commands.spawn((
                Node { id: from_id },
                Mesh2d(meshes.add(Circle::new(50.))),
                MeshMaterial2d(materials.add(Color::from(BLACK))),
                Transform::from_translation(Vec3::new(rng.random_range(-50f32..50f32), rng.random_range(-50f32..50f32), 1.0)),
                GlobalTransform::default(),
                Text2d::new(from_id.to_string()),
                text_font.clone(),
            )).id();
            ent
        });

        let to_ent = *node_map.entry(to_id).or_insert_with(|| {
            let ent = commands.spawn((
                Text2d::new(to_id.to_string()),
                text_font.clone(),
                Node { id: to_id },
                Mesh2d(meshes.add(Circle::new(50.))),
                MeshMaterial2d(materials.add(Color::from(BLACK))),
                Transform::from_translation(Vec3::new(rng.random_range(-50f32..50f32), rng.random_range(-50f32..50f32), 1.0)),
                GlobalTransform::default(),
            )).id();
            ent
        });

        commands.spawn((
            Edge { from: from_ent, to: to_ent, weight: parts.get(2).and_then(|s| s.parse().ok()).and_then(|n: f32| Some(n.ln())) },
            Mesh2d(meshes.add(Rectangle::new(0., 0.))),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::from(RED)))),
            Transform::default(),
            GlobalTransform::default(),
        ));
    }
    Ok(())
}

fn draw_edges(
    query_nodes: Query<(&Transform, &Node), Without<Edge>>,
    mut query_edges: Query<(&mut Transform, &mut Mesh2d, &Edge)>,
    mut meshes: ResMut<Assets<Mesh>>,
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
            
        }
    }
}

fn draw_nodes(
    mut query_nodes: Query<(Entity, &mut MeshMaterial2d<ColorMaterial>), With<Node>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    selected: Res<Selected>,
    state: Res<State>
) {
    // Reset all nodes to BLACK
    if let State::Dijstra {from, ref dist} = *state {
        let max = dist.iter().filter_map(|(k, v)|  if *v != f32::INFINITY {Some(OrderedFloat(*v))} else {None}).max().unwrap_or(OrderedFloat(0.)).to_f32();
        let start = GREEN;
        let end = BLACK;
        for (entity, mut mat) in query_nodes.iter_mut() {
            let val = (dist.get(&entity)).unwrap_or(&max) / max;

            if entity == from {
                *mat = MeshMaterial2d(materials.add(Color::from(GREEN)));
            } else {
                *mat = MeshMaterial2d(materials.add( Color::Srgba(Srgba::new(
                    start.red * (1.0 - val) + end.red * val,
                    start.green * (1.0 - val) + end.green * val,
                    start.blue * (1.0 - val) + end.blue * val,
                    1.
                ))));
            }
            if Some(entity) == selected.0 {
                *mat = MeshMaterial2d(materials.add(Color::from(RED)));
            }

        }
    } else {

        for (entity, mut mat) in query_nodes.iter_mut() {
            if Some(entity) == selected.0 {
                *mat = MeshMaterial2d(materials.add(Color::from(RED)));
            } else {
                *mat = MeshMaterial2d(materials.add(Color::from(BLACK)));
            }

        }
    }
}

fn apply_forces(
    mut query: Query<(Entity, &mut Transform, &Node)>,
    edge_query: Query<&Edge>,
    mut velocities: Local<HashMap<Entity, Vec2>>,
    mut prev_forces: Local<HashMap<Entity, Vec2>>,
    mut prev_global_speed: Local<f32>,
    mut timestep: Local<Option<f32>>,
    config: Res<Config>
) {
    if !config.enabled {
        return;
    }
    if *timestep == None {
        *timestep = Some(20.);
    }
    if let Some(timestep_u) = *timestep {
        if timestep_u > 1. {
            *timestep = Some(0.80 * timestep_u);
        }
    }
    let k_r = config.k_r;
    let k_w = config.k_w;
    let k_s = 0.1;
    let mut global_swinging = 0.0;
    let mut global_traction = 0.0;

    // --- 1. Gather node positions
    let mut positions = HashMap::new();
    for (ent, tf, _) in query.iter() {
        positions.insert(ent, tf.translation.truncate());
    }

    // --- 2. Compute attraction (edges)
    let mut a_forces = HashMap::new();
    for Edge { to: ent1, from: ent2, weight } in edge_query.iter() {
        let mut delta = positions[&ent2] - positions[&ent1];
        if let Some(w) = weight {
            delta *= w.powf(k_w);
        }
        *a_forces.entry(ent1).or_insert(Vec2::ZERO) -= delta;
        *a_forces.entry(ent2).or_insert(Vec2::ZERO) += delta;
    }

    // --- 3. Compute degree-weighted repulsion
    let degrees = compute_degrees(&edge_query);
    let mut r_forces = HashMap::new();
    for (&ent1, &pos1) in positions.iter() {
        for (&ent2, &pos2) in positions.iter() {
            if ent1 == ent2 { continue; }
            let delta = pos1 - pos2;
            let dist = delta.length().max(1e-6);
            let deg_mul = (degrees[&ent1] + 1) * (degrees[&ent2] + 1);
            *r_forces.entry(ent1).or_insert(Vec2::ZERO) +=
                delta.normalize() * (k_r * deg_mul as f32 / dist);
        }
    }

    // --- 4. Add gravity
    // for (&ent, &pos) in positions.iter() {
    //     *r_forces.entry(ent).or_insert(Vec2::ZERO) += -pos.normalize() * k_g * (degrees[&ent] as f32 + 1.);
    // }
    // --- 5. Combine forces
    let mut current_forces = HashMap::new();
    for (ent, _, _) in query.iter() {
        let curr_f = -a_forces.get(&ent).unwrap_or(&Vec2::ZERO)
            + r_forces.get(&ent).unwrap_or(&Vec2::ZERO);
        *current_forces.entry(ent).or_insert(Vec2::ZERO) = curr_f;
    }

    // --- 6. Calculate the global speed
    for (ent, curr_f) in current_forces.iter() {
        let prev_f = prev_forces.get(ent).unwrap_or(&Vec2::ZERO);
        let deg = (degrees[ent] + 1) as f32;

        let swinging = (*curr_f - *prev_f).length();
        let traction = ((*curr_f + *prev_f) * 0.5).length();

        global_swinging += deg * swinging;
        global_traction += deg * traction;
    }


    let global_speed = if global_swinging > 1e-6 {
        0.1 * (global_traction / global_swinging )* 0.5 + *prev_global_speed * 0.5
    } else {
        0.05 + *prev_global_speed * 0.5
    };
    let global_speed = global_speed.clamp(0.01, 10.0);
    *prev_global_speed = global_speed;

    // --- 7. Adaptive local speed
    for (ent, _, _) in query.iter() {
        let prev_f = prev_forces.entry(ent).or_insert(Vec2::ZERO);
        let curr_f = current_forces[&ent];

        let swinging = (*prev_f - curr_f).length();

        let mut s = k_s * global_speed / (1. + global_speed * swinging.sqrt());
        if s > 10. / curr_f.length() {
            s = 10. / curr_f.length();
        }
        let res = curr_f * s;
        *prev_forces.entry(ent).or_insert(Vec2::ZERO) = res;
        *velocities.entry(ent).or_insert(Vec2::ZERO) += res;
    }

    let damping = 0.95; // 0.8-0.9 is typical
    for v in velocities.values_mut() {
        *v *= damping;
    }

    // --- 8. Integrate positions
    for (ent, mut tf, _) in query.iter_mut() {
        if let Some(v) = velocities.get(&ent) {
            tf.translation += v.extend(0.0) * timestep.unwrap();
        }
    }
}

fn dijkstra(
    nodes: Query<Entity, With<Node>>,
    edges: Query<&Edge>,
    selected: Res<Selected>,
    mut state: ResMut<State>,
    mut reader: MessageReader<AlgoEvent>,
)  {
    for message in reader.read() {
        if *message == AlgoEvent::Dijstra && selected.0 != None {

            let mut graph: HashMap<Entity, Vec<(Entity, f32)>> = HashMap::new();
            for edge in edges.iter() {
                let w = edge.weight.unwrap_or(1.0);
                graph.entry(edge.from).or_default().push((edge.to, w));
                graph.entry(edge.to).or_default().push((edge.from, w));
            }

            let mut dist: HashMap<Entity, f32> = nodes.iter().map(|n| (n, f32::INFINITY)).collect();
            dist.insert(selected.0.unwrap(), 0.0);

            let mut heap = BinaryHeap::new();
            heap.push((Reverse(OrderedFloat(0.0)), selected.0.unwrap()));

            while let Some((Reverse(OrderedFloat(d)), u)) = heap.pop() {
                if d > dist[&u] { continue; }
                for &(v, w) in &graph[&u] {
                    let nd = d + w;
                    if nd < dist[&v] {
                        dist.insert(v, nd);
                        heap.push((Reverse(OrderedFloat(nd)), v));
                    }
                }
            }

            *state = State::Dijstra {
                dist,
                from: selected.0.unwrap()
            };
            break;
        }
    }
}

fn ui_system(mut egui_ctx: EguiContexts, mut config: ResMut<Config>, mut writer: MessageWriter<AlgoEvent>) -> Result {
    egui::Window::new("Physics settings").show(egui_ctx.ctx_mut()?, |ui| {
        ui.checkbox(&mut config.enabled, "Enable physics");
        ui.add(egui::Slider::new(&mut config.k_r, 0.0..=10000.0).text("Repulsion force"));
        ui.add(egui::Slider::new(&mut config.k_w, 0.0..=1.0).text("The weight of weights"));
    });
    egui::Window::new("Algorithms").show(egui_ctx.ctx_mut()?, |ui| {
        if ui.button("Get distances to all (dijstra)").clicked() {
            writer.write(AlgoEvent::Dijstra);
        }
        if ui.button("Get distances to all (bfs)").clicked() {
            writer.write(AlgoEvent::BFS);

        }
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
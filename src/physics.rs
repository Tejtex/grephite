use std::collections::HashMap;

use bevy::prelude::*;

use crate::components::*;
use crate::compute_degrees;

pub(crate) fn apply_forces(
    mut query: Query<(Entity, &mut Transform, &GNode)>,
    edge_query: Query<&GEdge>,
    mut velocities: Local<HashMap<Entity, Vec2>>,
    mut prev_forces: Local<HashMap<Entity, Vec2>>,
    mut prev_global_speed: Local<f32>,
    mut timestep: Local<Option<f32>>,
    config: Res<Config>,
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
    let k_g = config.k_g;
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
    for GEdge {
        to: ent1,
        from: ent2,
        weight,
    } in edge_query.iter()
    {
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
            if ent1 == ent2 {
                continue;
            }
            let delta = pos1 - pos2;
            let dist = delta.length().max(1e-6);
            let deg_mul =
                (degrees.get(&ent1).unwrap_or(&0) + 1) * (degrees.get(&ent2).unwrap_or(&0) + 1);
            *r_forces.entry(ent1).or_insert(Vec2::ZERO) +=
                delta.normalize() * (k_r * deg_mul as f32 / dist);
        }
    }

    // --- 4. Add gravity
    for (&ent, &pos) in positions.iter() {
        *r_forces.entry(ent).or_insert(Vec2::ZERO) +=
            -pos.normalize() * k_g * (*degrees.get(&ent).unwrap_or(&0) as f32 + 1.) * pos.length();
    }
    // --- 5. Combine forces
    let mut current_forces = HashMap::new();
    for (ent, _, _) in query.iter() {
        let curr_f =
            -a_forces.get(&ent).unwrap_or(&Vec2::ZERO) + r_forces.get(&ent).unwrap_or(&Vec2::ZERO);
        *current_forces.entry(ent).or_insert(Vec2::ZERO) = curr_f;
    }

    // --- 6. Calculate the global speed
    for (ent, curr_f) in current_forces.iter() {
        let prev_f = prev_forces.get(ent).unwrap_or(&Vec2::ZERO);
        let deg = (degrees.get(ent).unwrap_or(&0) + 1) as f32;

        let swinging = (*curr_f - *prev_f).length();
        let traction = ((*curr_f + *prev_f) * 0.5).length();

        global_swinging += deg * swinging;
        global_traction += deg * traction;
    }

    let global_speed = if global_swinging > 1e-6 {
        0.1 * (global_traction / global_swinging) * 0.5 + *prev_global_speed * 0.5
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

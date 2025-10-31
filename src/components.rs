use std::collections::HashMap;

use bevy::prelude::*;

#[derive(Component)]
pub struct GNode {
    pub id: usize,
}

#[derive(Component)]
pub struct GEdge {
    pub from: Entity,
    pub to: Entity,
    pub weight: Option<f32>,
}
#[derive(Resource, Default)]
pub struct DragState {
    pub dragging: Option<Entity>,
    pub offset: Vec2, // offset from mouse to node center
}
#[derive(Resource)]
pub struct Config {
    pub k_r: f32,
    pub k_w: f32,
    pub k_g: f32,
    pub enabled: bool,
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppMode {
    #[default]
    View,
    Edit,
    Script,
}

#[derive(Resource)]
pub struct Selected(pub Option<Entity>);

#[derive(Resource, Default)]
pub struct Graph {
    pub adj: HashMap<Entity, Vec<Entity>>,
    pub edges: Vec<Entity>,
    pub curr_id: usize,
}

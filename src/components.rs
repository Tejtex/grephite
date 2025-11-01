use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use bevy::prelude::*;
use mlua::Lua;

#[derive(Component)]
pub struct GNode {
    pub id: usize,
}

#[derive(Component)]
pub struct GEdge {
    pub from: Entity,
    pub to: Entity,
}
#[derive(Resource, Default)]
pub struct DragState {
    pub dragging: Option<Entity>,
    pub offset: Vec2, // offset from mouse to node center
}
#[derive(Resource)]
pub struct Config {
    pub k_r: f32,
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
#[derive(Resource, Default)]
pub struct EdgeCreation {
    pub from: Option<Entity>, // first node clicked
    pub temp_line: Option<Entity>, // temporary line entity
}

#[derive(Resource, Default)]
pub struct DeletionRequest {
    pub node: Option<Entity>,
    pub edge: Option<Entity>,
}
#[derive(Resource)]
pub struct ScriptRuntime {
    pub(crate) lua: Lua,
}
#[derive(Message, Clone)]
pub struct ScriptCommand {
    pub node_raw: u64,
    pub color_hex: String,
}
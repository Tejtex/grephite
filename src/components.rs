use bevy::prelude::*;
use mlua::{Lua, Thread, ThreadStatus, UserData, UserDataMethods};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    pub scripts_dir: String,
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

#[derive(Resource, Default, Clone)]
pub struct Graph {
    pub adj: HashMap<Entity, Vec<Entity>>,
    pub edges: Vec<Entity>,
    pub curr_id: usize,
}
#[derive(Resource, Default)]
pub struct EdgeCreation {
    pub from: Option<Entity>,      // first node clicked
    pub temp_line: Option<Entity>, // temporary line entity
}

#[derive(Resource, Default)]
pub struct DeletionRequest {
    pub node: Option<Entity>,
    pub edge: Option<Entity>,
}
#[derive(Resource, Default)]
pub struct LuaManager {
    pub active_script: Option<LuaThreadState>,
}

#[derive(Message, Clone)]
pub enum ScriptCommand {
    SetColor(u64, String),
    ResetColor(u64),
}

#[derive(Message)]
pub struct ExecLuaScript {
    pub code: String,
}

pub struct LuaThreadState {
    pub lua: Lua,
    pub thread: Thread,
    pub event_buffer: Arc<Mutex<Vec<ScriptCommand>>>,
    pub running: bool,
    pub speed: f32,
}

#[derive(Message)]
pub struct StepLua;

#[derive(Resource)]
pub struct NodeColors {
    pub colors: HashMap<Entity, Color>,
}

#[derive(Clone)]
pub struct LuaGraph {
    pub inner: Arc<Mutex<Graph>>,
}

impl UserData for LuaGraph {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| {
            let g = this.inner.lock().unwrap();
            Ok(g.adj.len())
        });

        methods.add_method("get_nodes", |lua, this, ()| {
            let g = this.inner.lock().unwrap();
            let tbl = lua.create_table()?;
            for (i, e) in g.adj.keys().enumerate() {
                tbl.set(i + 1, e.to_bits())?;
            }
            Ok(tbl)
        });

        methods.add_method("get_neighbours", |lua, this, node: u64| {
            let g = this.inner.lock().unwrap();
            let tbl = lua.create_table()?;
            for (i, n) in g
                .adj
                .get(&Entity::from_bits(node))
                .unwrap()
                .iter()
                .enumerate()
            {
                tbl.set(i + 1, n.to_bits())?;
            }
            Ok(tbl)
        })
    }
}

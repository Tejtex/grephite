use std::sync::*;

use bevy::prelude::*;
use mlua::prelude::*;

use mlua::{Thread, Value};

use crate::components::*;

pub fn spawn_lua_scripts(
    mut manager: ResMut<LuaManager>,
    mut new_script: MessageReader<ExecLuaScript>,
    graph: Res<Graph>,
) {
    for mes in new_script.read() {
        let lua = Lua::new();
        let ev = Arc::new(Mutex::new(Vec::<ScriptCommand>::new()));

        let set_color_buf = Arc::clone(&ev);
        let set_color = lua
            .create_function(move |_, (node, color): (u64, String)| {
                set_color_buf
                    .lock()
                    .unwrap()
                    .push(ScriptCommand::SetColor(node, color));
                Ok(())
            })
            .unwrap();

        let reset_color_buf = Arc::clone(&ev);
        let reset_color = lua
            .create_function(move |_, node: u64| {
                reset_color_buf
                    .lock()
                    .unwrap()
                    .push(ScriptCommand::ResetColor(node));
                Ok(())
            })
            .unwrap();

        let globals = lua.globals();
        globals.set("set_color", set_color).unwrap();
        globals.set("reset_color", reset_color).unwrap();
        let lua_graph = LuaGraph {
            inner: Arc::new(Mutex::new(graph.clone())),
        };
        globals.set("graph", lua_graph).unwrap();

        let func = lua.load(&mes.code).into_function().unwrap();
        let thread = lua.create_thread(func).unwrap();

        manager.active_script = Some(LuaThreadState {
            lua,
            thread,
            event_buffer: ev,
            running: false,
            speed: 1.,
        });
    }
}

pub fn run_lua_scripts(mut manager: ResMut<LuaManager>, mut step_lua: MessageReader<StepLua>) {
    if let Some(state) = &mut manager.active_script {
        // sprawdzamy, czy Step dotyczy bieżącego skryptu
        let do_step = step_lua.read().next().is_some();
        if do_step {
            if state.thread.status() != LuaThreadStatus::Finished {
                if let Err(e) = state.thread.resume::<Value>(Value::NULL) {
                    eprintln!("Lua error: {e}");
                }
            } else {
                // jeśli coroutine się skończył, usuwamy aktywny skrypt
                manager.active_script = None;
            }
        }
    }
}

pub fn flush_lua_events(mut manager: ResMut<LuaManager>, mut writer: MessageWriter<ScriptCommand>) {
    if let Some(state) = &mut manager.active_script {
        let mut buf = state.event_buffer.lock().unwrap();
        for ev in buf.drain(..) {
            writer.write(ev);
        }
    }
}

pub fn exec_lua_events(mut reader: MessageReader<ScriptCommand>, mut colors: ResMut<NodeColors>) {
    for mes in reader.read() {
        match mes {
            ScriptCommand::SetColor(node, color) => {
                *colors
                    .colors
                    .entry(Entity::from_bits(*node))
                    .or_insert(Color::BLACK) = color_from_hex(color).unwrap();
            }
            ScriptCommand::ResetColor(node) => {
                *colors
                    .colors
                    .entry(Entity::from_bits(*node))
                    .or_insert(Color::BLACK) = Color::BLACK;
            }
        }
    }
}

pub fn color_from_hex(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        3 => {
            // e.g. "#0f0" → "00ff00"
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };

    Some(Color::srgb_u8(r, g, b).with_alpha(a as f32 / 255.0))
}

pub fn auto_run(
    mut timel: Local<f32>,
    mut manager: ResMut<LuaManager>,
    mut writer: MessageWriter<StepLua>,
    time: Res<Time>,
) {
    if let Some(active) = &manager.active_script {
        if !active.running {
            return;
        }
        *timel += time.delta_secs();
        let secs = 1. / active.speed;
        if *timel < secs {
            return;
        }
        writer.write(StepLua);
        *timel = 0.;
    }
}

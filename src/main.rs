mod abilities;
mod audio;
mod boss;
mod callouts;
mod chest;
mod destruct;
mod elites;
mod content;
mod coop;
mod corpses;
mod game;
mod input;
mod math;
mod modes;
mod nav;
mod net;
mod outbreak_interact;
mod ping;
mod render;
mod render_hub;
mod render_light;
mod render_outbreak;
mod render_siege;
mod render_map;
mod secrets;
mod status;
mod tutorial;
mod upgrades;
mod weather;
mod world;

use game::{Game, Phase};
use math::v2;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Event, HtmlCanvasElement, KeyboardEvent, MouseEvent, TouchEvent,
};

#[wasm_bindgen]
extern "C" {
    /// JS -> Rust command queue (menus, settings). Returns undefined when empty.
    #[wasm_bindgen(js_namespace = window, js_name = uhPoll)]
    fn ui_poll() -> Option<String>;
    /// Rust -> JS notifications (game over, pause, ...).
    #[wasm_bindgen(js_namespace = window, js_name = uhEvent)]
    fn ui_event(kind: &str, data: &str);
    /// Co-op: send bytes to a player slot over WebRTC (reliable or not).
    #[wasm_bindgen(js_namespace = window, js_name = uhNetSend)]
    fn net_send(slot: u8, reliable: bool, data: &[u8]);
    /// Co-op: next received message as `{ from, data: Uint8Array }`, or undefined.
    #[wasm_bindgen(js_namespace = window, js_name = uhNetPoll)]
    fn net_poll() -> JsValue;
}

fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    f: impl FnMut(E) + 'static,
) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let _ = target.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref());
    cb.forget();
}

/// `KeyboardEvent.code`, or None for synthetic key events (e.g. Chrome autofill
/// dispatches plain `Event`s named keydown/keyup that have no `code`; calling
/// the typed getter on those throws inside Rust and wedges the game state).
fn key_code(e: &KeyboardEvent) -> Option<String> {
    js_sys::Reflect::get(e, &"code".into())
        .ok()
        .and_then(|v| v.as_string())
        .filter(|c| !c.is_empty())
}

fn is_text_input(e: &Event) -> bool {
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .map(|el| {
            let tag = el.tag_name();
            tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT"
        })
        .unwrap_or(false)
}

fn main() {
    if let Err(e) = run() {
        web_sys::console::error_1(&e);
    }
}

fn run() -> Result<(), JsValue> {
    // Log Rust panics as readable console errors (release builds abort on panic).
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("Undead Horizon crashed: {info}").into());
        // the frame loop is dead after a panic: tell the UI so it can offer a reload
        ui_event("crash", &info.to_string());
    }));
    let win = window();
    let doc = win.document().expect("no document");
    let canvas: HtmlCanvasElement = doc
        .get_element_by_id("game")
        .expect("#game canvas missing")
        .dyn_into()?;
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &"alpha".into(), &false.into())?;
    js_sys::Reflect::set(&opts, &"desynchronized".into(), &true.into())?;
    let ctx: CanvasRenderingContext2d = canvas
        .get_context_with_context_options("2d", &opts)?
        .expect("2d context")
        .dyn_into()?;

    let seed = (js_sys::Math::random() * 1e15) as u64 ^ (js_sys::Date::now() as u64);
    let game = Rc::new(RefCell::new(Game::new(seed)));
    let win_t: web_sys::EventTarget = win.clone().into();
    let canvas_t: web_sys::EventTarget = canvas.clone().into();

    // ---------------- keyboard
    {
        let g = game.clone();
        listen(&win_t, "keydown", move |e: KeyboardEvent| {
            if is_text_input(&e) {
                return;
            }
            let Some(code) = key_code(&e) else { return };
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            g.audio.unlock();
            let playing = matches!(g.phase, Phase::Playing | Phase::LevelUp | Phase::Paused);
            if playing
                && matches!(
                    code.as_str(),
                    "Space" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Tab"
                )
            {
                e.prevent_default();
            }
            set_key(&mut g, &code, true);
            let repeat = js_sys::Reflect::get(&e, &"repeat".into())
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !repeat {
                g.input.pressed.push(code);
            }
        });
    }
    {
        let g = game.clone();
        listen(&win_t, "keyup", move |e: KeyboardEvent| {
            let Some(code) = key_code(&e) else { return };
            if let Ok(mut g) = g.try_borrow_mut() {
                set_key(&mut g, &code, false);
            }
        });
    }

    // ---------------- mouse
    {
        let g = game.clone();
        listen(&win_t, "mousemove", move |e: MouseEvent| {
            // read event fields before borrowing: a throwing getter must never
            // leave the game borrowed
            let m = v2(e.client_x() as f32, e.client_y() as f32);
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            g.input.mouse = m;
            g.input.mouse_seen = true;
        });
    }
    {
        let g = game.clone();
        listen(&canvas_t, "mousedown", move |e: MouseEvent| {
            let m = v2(e.client_x() as f32, e.client_y() as f32);
            let button = e.button();
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            g.audio.unlock();
            g.input.mouse = m;
            g.input.mouse_seen = true;
            g.input.is_touch = false;
            match button {
                0 => {
                    g.input.mouse_down = true;
                    let m = g.input.mouse;
                    g.input.clicks.push(m);
                }
                2 => g.input.ult_req = true,
                _ => {}
            }
        });
    }
    {
        let g = game.clone();
        listen(&win_t, "mouseup", move |e: MouseEvent| {
            if e.button() == 0 {
                if let Ok(mut g) = g.try_borrow_mut() {
                    g.input.mouse_down = false;
                }
            }
        });
    }
    listen(&canvas_t, "contextmenu", |e: MouseEvent| {
        e.prevent_default()
    });
    {
        let g = game.clone();
        listen(&win_t, "pointerdown", move |_e: Event| {
            if let Ok(mut g) = g.try_borrow_mut() {
                g.audio.unlock();
            }
        });
    }

    // ---------------- focus: auto-pause so you never die while tabbed out
    {
        let g = game.clone();
        listen(&win_t, "blur", move |_e: Event| {
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            g.input = game::Input {
                is_touch: g.input.is_touch,
                mouse: g.input.mouse,
                mouse_seen: g.input.mouse_seen,
                ..Default::default()
            };
            g.pause();
        });
    }
    {
        let g = game.clone();
        let d2 = doc.clone();
        let doc_t: web_sys::EventTarget = doc.clone().into();
        listen(&doc_t, "visibilitychange", move |_e: Event| {
            if d2.hidden() {
                if let Ok(mut g) = g.try_borrow_mut() {
                    g.pause();
                }
            }
        });
    }

    // ---------------- touch: left half = floating joystick, right half tap = dash
    {
        let g = game.clone();
        listen(&canvas_t, "touchstart", move |e: TouchEvent| {
            e.prevent_default();
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            g.audio.unlock();
            g.input.is_touch = true;
            let w = window()
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(800.0) as f32;
            let list = e.changed_touches();
            for i in 0..list.length() {
                if let Some(t) = list.item(i) {
                    let p = v2(t.client_x() as f32, t.client_y() as f32);
                    if g.phase == Phase::LevelUp {
                        g.input.clicks.push(p);
                        continue;
                    }
                    // Siege: taps pick from the build bar or place the selected defense
                    if g.siege.is_some() && g.phase == Phase::Playing && (p.y > g.screen.y - 260.0 || (g.siege.as_ref().is_some_and(|s| s.build_sel.is_some()) && p.x >= w * 0.5)) {
                        g.input.clicks.push(p);
                    }
                    if p.x < w * 0.5 && g.input.joy_id.is_none() {
                        g.input.joy_id = Some(t.identifier());
                        g.input.joy_origin = p;
                        g.input.joy_pos = p;
                        g.input.touch_active = true;
                        g.input.touch_move = math::V2::ZERO;
                    } else if p.x >= w * 0.5 {
                        g.input.dash_req = true;
                    }
                }
            }
        });
    }
    {
        let g = game.clone();
        listen(&canvas_t, "touchmove", move |e: TouchEvent| {
            e.prevent_default();
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            let list = e.changed_touches();
            for i in 0..list.length() {
                if let Some(t) = list.item(i) {
                    if Some(t.identifier()) == g.input.joy_id {
                        let p = v2(t.client_x() as f32, t.client_y() as f32);
                        let mut d = p - g.input.joy_origin;
                        let max = 60.0;
                        if d.len() > max {
                            // floating stick: drag the origin along
                            let excess = d.norm() * (d.len() - max);
                            g.input.joy_origin += excess;
                            d = p - g.input.joy_origin;
                        }
                        g.input.joy_pos = p;
                        g.input.touch_move = d * (1.0 / max);
                    }
                }
            }
        });
    }
    for name in ["touchend", "touchcancel"] {
        let g = game.clone();
        listen(&canvas_t, name, move |e: TouchEvent| {
            e.prevent_default();
            let Ok(mut g) = g.try_borrow_mut() else {
                return;
            };
            let list = e.changed_touches();
            for i in 0..list.length() {
                if let Some(t) = list.item(i) {
                    if Some(t.identifier()) == g.input.joy_id {
                        g.input.joy_id = None;
                        g.input.touch_active = false;
                        g.input.touch_move = math::V2::ZERO;
                    }
                }
            }
        });
    }

    // ---------------- main loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let f2 = f.clone();
    let g = game.clone();
    *f2.borrow_mut() = Some(Closure::new(move |ts: f64| {
        let w = window();
        let cw = w
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0) as f32;
        let chh = w
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(600.0) as f32;
        let dpr = (w.device_pixel_ratio() as f32).clamp(1.0, 2.0);
        let pw = (cw * dpr).round() as u32;
        let ph = (chh * dpr).round() as u32;
        if canvas.width() != pw || canvas.height() != ph {
            canvas.set_width(pw);
            canvas.set_height(ph);
        }
        // Never call back into JS while the game is borrowed: JS handlers can
        // synchronously trigger DOM events whose Rust listeners need the game.
        let events = match g.try_borrow_mut() {
            Ok(mut g) => {
                while let Some(cmd) = ui_poll() {
                    handle_cmd(&mut g, &cmd);
                }
                if g.net.is_some() {
                    loop {
                        let m = net_poll();
                        if m.is_undefined() || m.is_null() {
                            break;
                        }
                        let from = js_sys::Reflect::get(&m, &"from".into())
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(255.0) as u8;
                        if let Ok(d) = js_sys::Reflect::get(&m, &"data".into()) {
                            let bytes = js_sys::Uint8Array::new(&d).to_vec();
                            if from < net::MAX_PLAYERS as u8 && g.net_in.len() < 4096 {
                                g.net_in.push((from, bytes));
                            }
                        }
                    }
                }
                g.screen = v2(cw, chh);
                g.frame(ts);
                g.render(&ctx, cw, chh, dpr);
                let out = std::mem::take(&mut g.net_out);
                (std::mem::take(&mut g.ui_out), out)
            }
            Err(_) => (Vec::new(), Vec::new()),
        };
        let (events, net_out) = events;
        for (slot, reliable, data) in net_out {
            net_send(slot, reliable, &data);
        }
        for (kind, data) in events {
            ui_event(kind, &data);
        }
        let _ = w.request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }));
    win.request_animation_frame(f2.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    ui_event("ready", "");
    Ok(())
}

fn set_key(g: &mut Game, code: &str, down: bool) {
    let mut moved = false;
    for a in g.bindings.actions(code).to_vec() {
        if a == input::Action::Interact {
            if g.access.toggle_use {
                if down {
                    g.input.use_down = !g.input.use_down;
                }
            } else {
                g.input.use_down = down;
            }
        }
        if !a.is_held() {
            continue;
        }
        let i = &mut g.input;
        match a {
            input::Action::Up => i.up = down,
            input::Action::Down => i.down = down,
            input::Action::Left => i.left = down,
            input::Action::Right => i.right = down,
            _ => continue,
        }
        moved = true;
    }
    if down && moved {
        g.input.is_touch = false;
    }
}

fn handle_cmd(g: &mut Game, cmd: &str) {
    let mut parts = cmd.splitn(2, '|');
    let head = parts.next().unwrap_or("");
    let a = parts.next().unwrap_or("");
    match head {
        "start" => g.start(a),
        "coop" => g.start_coop(a),
        "peerleft" => g.peer_left(a.parse().unwrap_or(255)),
        // party members for the lobby showcase: name~skin~hat~gun|...
        "lobbymates" => {
            g.lobby_mates = a
                .split('|')
                .filter(|s| !s.is_empty())
                .take(2)
                .map(|m| {
                    let f: Vec<&str> = m.split('~').collect();
                    (
                        f.first()
                            .map(|s| s.chars().take(16).collect())
                            .unwrap_or_default(),
                        content::skin_index(f.get(1).copied().unwrap_or("")),
                        content::hat_index(f.get(2).copied().unwrap_or("")),
                        content::gun_index(f.get(3).copied().unwrap_or("")),
                    )
                })
                .collect();
        }
        "equip" => g.apply_opts(a),
        "load" => g.load_save(a),
        "save" => {
            let data = g.save_string();
            g.ui_out.push(("saved", data));
        }
        "resume" => g.resume(),
        "pause" => g.pause(),
        "menu" => g.to_menu(),
        "vol" => g.audio.set_volume(a.parse().unwrap_or(0.7)),
        "music" => g.audio.set_music_volume(a.parse().unwrap_or(0.5)),
        "tune" => {
            g.audio.lobby_tune = a.parse::<usize>().unwrap_or(0) % audio::LOBBY_TUNES.len();
            g.ui_out.push(("tune", audio::LOBBY_TUNES[g.audio.lobby_tune].name.to_string()));
        }
        "autoaim" => g.auto_aim = a == "1",
        "access" => g.access.apply(a),
        "lookahead" => g.lookahead = a.parse::<f32>().unwrap_or(1.0).clamp(0.0, 2.0),
        "dash" => g.input.dash_req = true,
        "ult" => g.input.ult_req = true,
        "touch" => g.input.is_touch = a == "1",
        // the touch USE button is held to rebuild barricades
        "use" => {
            g.input.use_down = a == "1";
            if a == "1" {
                g.input.actions.push(input::Action::Interact);
            }
        }
        // rebind keys: action=Code,Code;...  (empty = reset to defaults)
        "bind" => {
            if a.is_empty() {
                g.bindings = input::Bindings::default();
            } else if a == "?" {
                // just report the current bindings
            } else {
                g.bindings.apply(a);
            }
            let spec = g.bindings.to_spec();
            g.ui_out.push(("bindings", spec));
        }
        // test hooks for e2e screenshots
        "dev" => {
            if a == "chest" && g.phase == Phase::Playing {
                g.open_chest();
            }
            if a == "kinds" && g.phase == Phase::Playing {
                let p = g.player.pos;
                use game::ZKind::*;
                for (k, kind) in [Crawler, Screamer, Armored, Bat, Burrower].iter().enumerate() {
                    g.spawn_zombie(*kind, p + math::V2::from_angle(k as f32 * 1.25 + 0.3) * 230.0);
                }
            }
            if let Some(v) = a.strip_prefix("tp=") {
                let xy: Vec<f32> = v.split(',').filter_map(|n| n.parse().ok()).collect();
                if xy.len() == 2 {
                    g.player.pos = math::v2(xy[0], xy[1]);
                    g.player.prev = g.player.pos;
                }
            }
            if let Some(v) = a.strip_prefix("points=") {
                if let Some(o) = g.ob.as_mut() {
                    o.points = v.parse().unwrap_or(0);
                }
            }
            if let Some(v) = a.strip_prefix("boss=") {
                let p = g.player.pos + math::v2(260.0, -60.0);
                let var = content::BossVar::from_u8(v.parse().unwrap_or(0));
                g.spawn_boss(var, p);
                g.boss_intro(var);
            }
            if let Some(v) = a.strip_prefix("biome=") {
                'find: for r in 1..12i32 {
                    for by in -r..=r {
                        for bx in -r..=r {
                            if format!("{:?}", world::biome_of_cell(bx, by)).to_lowercase() == v {
                                let p = math::v2(bx as f32 * world::BIOME_CELL, by as f32 * world::BIOME_CELL);
                                g.player.pos = world::resolve(p, 16.0).0;
                                g.player.prev = g.player.pos;
                                break 'find;
                            }
                        }
                    }
                }
            }
            if let Some(v) = a.strip_prefix("sky=") {
                let (sky, clock) = v.split_once(',').unwrap_or((v, "0.25"));
                g.weather.sky = match sky {
                    "rain" => weather::Sky::Rain,
                    "fog" => weather::Sky::Fog,
                    _ => weather::Sky::Clear,
                };
                g.weather.k = 1.0;
                g.weather.next = 999.0;
                g.weather.clock = clock.parse().unwrap_or(0.25);
            }
            if a == "horde" && g.phase == Phase::Playing {
                let p = g.player.pos;
                for k in 0..800 {
                    let ang = k as f32 * 0.37;
                    let r = 250.0 + (k % 30) as f32 * 14.0;
                    g.spawn_zombie(game::ZKind::ALL[k % 5], p + math::V2::from_angle(ang) * r);
                }
            }
            if a == "elites" && g.phase == Phase::Playing {
                let p = g.player.pos;
                for (k, af) in elites::AFFIXES.iter().enumerate() {
                    let kind = if k % 2 == 0 { game::ZKind::Brute } else { game::ZKind::Walker };
                    g.spawn_zombie(kind, p + math::V2::from_angle(k as f32 * 1.05) * 260.0);
                    g.make_elite(*af);
                }
            }
        }
        // Home Base: vendor lines, other players, chat bubbles, closing a stall
        "npcsay" => {
            if let Some((id, text)) = a.split_once('|') {
                g.npc_say(id, text);
            }
        }
        "hubmates" => g.set_hub_mates(a),
        "hubbubble" => {
            if let Some((name, text)) = a.split_once('|') {
                g.hub_bubble(name, text);
            }
        }
        "hubclose" => {
            if let Some(h) = g.hub.as_mut() {
                h.open = None;
            }
        }
        // gamepad sticks: mx,my,ax,ay,fire (sent every frame while a pad is in use)
        "pad" => {
            let v: Vec<f32> = a.split(',').filter_map(|x| x.parse().ok()).collect();
            if v.len() >= 5 {
                g.input.pad_active = true;
                g.input.pad_move = math::v2(v[0], v[1]);
                g.input.pad_aim = math::v2(v[2], v[3]);
                g.input.pad_fire = v[4] > 0.5;
                g.input.is_touch = false;
            } else {
                g.input.pad_active = false;
            }
        }
        // a one-shot action from a gamepad or an on-screen button
        "act" => {
            if let Some(act) = input::Action::from_id(a) {
                g.input.actions.push(act);
            }
        }
        // one-shot UI sounds from the HTML layer (party lobby)
        "sfx" => {
            g.audio.unlock();
            match a {
                "ready" => g.audio.play(audio::Sfx::ReadyUp),
                "start" => g.audio.play(audio::Sfx::MatchStart),
                "click" => g.audio.play(audio::Sfx::UiSelect),
                "chat" => g.audio.play(audio::Sfx::UiMove),
                _ => {}
            }
        }
        _ => {}
    }
}

mod audio;
mod content;
mod game;
mod math;
mod render;
mod upgrades;
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
            let code = e.code();
            let mut g = g.borrow_mut();
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
            if !e.repeat() {
                g.input.pressed.push(code);
            }
        });
    }
    {
        let g = game.clone();
        listen(&win_t, "keyup", move |e: KeyboardEvent| {
            set_key(&mut g.borrow_mut(), &e.code(), false);
        });
    }

    // ---------------- mouse
    {
        let g = game.clone();
        listen(&win_t, "mousemove", move |e: MouseEvent| {
            let mut g = g.borrow_mut();
            g.input.mouse = v2(e.client_x() as f32, e.client_y() as f32);
            g.input.mouse_seen = true;
        });
    }
    {
        let g = game.clone();
        listen(&canvas_t, "mousedown", move |e: MouseEvent| {
            let mut g = g.borrow_mut();
            g.audio.unlock();
            g.input.mouse = v2(e.client_x() as f32, e.client_y() as f32);
            g.input.mouse_seen = true;
            g.input.is_touch = false;
            match e.button() {
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
                g.borrow_mut().input.mouse_down = false;
            }
        });
    }
    listen(&canvas_t, "contextmenu", |e: MouseEvent| {
        e.prevent_default()
    });
    {
        let g = game.clone();
        listen(&win_t, "pointerdown", move |_e: Event| {
            g.borrow_mut().audio.unlock();
        });
    }

    // ---------------- focus: auto-pause so you never die while tabbed out
    {
        let g = game.clone();
        listen(&win_t, "blur", move |_e: Event| {
            let mut g = g.borrow_mut();
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
                g.borrow_mut().pause();
            }
        });
    }

    // ---------------- touch: left half = floating joystick, right half tap = dash
    {
        let g = game.clone();
        listen(&canvas_t, "touchstart", move |e: TouchEvent| {
            e.prevent_default();
            let mut g = g.borrow_mut();
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
            let mut g = g.borrow_mut();
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
            let mut g = g.borrow_mut();
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
        {
            let mut g = g.borrow_mut();
            while let Some(cmd) = ui_poll() {
                handle_cmd(&mut g, &cmd);
            }
            g.screen = v2(cw, chh);
            g.frame(ts);
            g.render(&ctx, cw, chh, dpr);
            for (kind, data) in std::mem::take(&mut g.ui_out) {
                ui_event(kind, &data);
            }
        }
        let _ = w.request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }));
    win.request_animation_frame(f2.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    ui_event("ready", "");
    Ok(())
}

fn set_key(g: &mut Game, code: &str, down: bool) {
    let i = &mut g.input;
    match code {
        "KeyW" | "ArrowUp" | "KeyZ" => i.up = down,
        "KeyS" | "ArrowDown" => i.down = down,
        "KeyA" | "ArrowLeft" => i.left = down,
        "KeyD" | "ArrowRight" => i.right = down,
        _ => return,
    }
    if down {
        i.is_touch = false;
    }
}

fn handle_cmd(g: &mut Game, cmd: &str) {
    let mut parts = cmd.splitn(2, '|');
    let head = parts.next().unwrap_or("");
    let a = parts.next().unwrap_or("");
    match head {
        "start" => g.start(a),
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
        "autoaim" => g.auto_aim = a == "1",
        "dash" => g.input.dash_req = true,
        "ult" => g.input.ult_req = true,
        "touch" => g.input.is_touch = a == "1",
        _ => {}
    }
}

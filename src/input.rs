//! Input actions and key bindings. Key handlers translate `KeyboardEvent.code`
//! into `Action`s through a rebindable table; gamepads and touch buttons push
//! actions directly. The sim only ever sees actions.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Dash,
    Ult,
    Interact,
    Ping,
    Pause,
    Ability,
    AutoAim,
    Minimap,
    Music,
    Fps,
    Pick1,
    Pick2,
    Pick3,
    Reroll,
    Banish,
    Skip,
    Confirm,
    Prev,
    Next,
}

pub const ACTIONS: [Action; 23] = [
    Action::Up,
    Action::Down,
    Action::Left,
    Action::Right,
    Action::Dash,
    Action::Ult,
    Action::Interact,
    Action::Ping,
    Action::Pause,
    Action::Ability,
    Action::AutoAim,
    Action::Minimap,
    Action::Music,
    Action::Fps,
    Action::Pick1,
    Action::Pick2,
    Action::Pick3,
    Action::Reroll,
    Action::Banish,
    Action::Skip,
    Action::Confirm,
    Action::Prev,
    Action::Next,
];

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::Up => "up",
            Action::Down => "down",
            Action::Left => "left",
            Action::Right => "right",
            Action::Dash => "dash",
            Action::Ult => "ult",
            Action::Interact => "interact",
            Action::Ping => "ping",
            Action::Pause => "pause",
            Action::Ability => "ability",
            Action::AutoAim => "autoaim",
            Action::Minimap => "minimap",
            Action::Music => "music",
            Action::Fps => "fps",
            Action::Pick1 => "pick1",
            Action::Pick2 => "pick2",
            Action::Pick3 => "pick3",
            Action::Reroll => "reroll",
            Action::Banish => "banish",
            Action::Skip => "skip",
            Action::Confirm => "confirm",
            Action::Prev => "prev",
            Action::Next => "next",
        }
    }

    pub fn from_id(s: &str) -> Option<Action> {
        ACTIONS.iter().copied().find(|a| a.id() == s)
    }

    /// Movement actions are held, everything else fires on press.
    pub fn is_held(self) -> bool {
        matches!(self, Action::Up | Action::Down | Action::Left | Action::Right)
    }

    fn defaults(self) -> &'static [&'static str] {
        match self {
            Action::Up => &["KeyW", "ArrowUp", "KeyZ"],
            Action::Down => &["KeyS", "ArrowDown"],
            Action::Left => &["KeyA", "ArrowLeft"],
            Action::Right => &["KeyD", "ArrowRight"],
            Action::Dash => &["Space", "ShiftLeft", "ShiftRight"],
            Action::Ult => &["KeyQ", "KeyE"],
            Action::Interact => &["KeyE"],
            Action::Ping => &["KeyG"],
            Action::Pause => &["Escape", "KeyP"],
            Action::Ability => &["KeyR"],
            Action::AutoAim => &["KeyF"],
            Action::Minimap => &["KeyN"],
            Action::Music => &["KeyM"],
            Action::Fps => &["Backquote"],
            Action::Pick1 => &["Digit1", "Numpad1"],
            Action::Pick2 => &["Digit2", "Numpad2"],
            Action::Pick3 => &["Digit3", "Numpad3"],
            Action::Reroll => &["KeyR"],
            Action::Banish => &["KeyB"],
            Action::Skip => &["KeyX"],
            Action::Confirm => &["Enter", "Space"],
            Action::Prev => &["ArrowLeft", "KeyA", "ArrowUp"],
            Action::Next => &["ArrowRight", "KeyD", "ArrowDown"],
        }
    }
}

/// code -> actions, in priority order (the first action that applies wins).
pub struct Bindings {
    codes: Vec<(String, Vec<Action>)>,
}

impl Default for Bindings {
    fn default() -> Self {
        let mut b = Bindings { codes: Vec::new() };
        // Interact is listed before Ult so E opens doors when there is one.
        let order = [Action::Interact];
        for a in order.iter().chain(ACTIONS.iter().filter(|a| !order.contains(a))) {
            for c in a.defaults() {
                b.add(c, *a);
            }
        }
        b
    }
}

impl Bindings {
    fn add(&mut self, code: &str, a: Action) {
        match self.codes.iter_mut().find(|(c, _)| c == code) {
            Some((_, v)) => {
                if !v.contains(&a) {
                    v.push(a)
                }
            }
            None => self.codes.push((code.to_string(), vec![a])),
        }
    }

    pub fn actions(&self, code: &str) -> &[Action] {
        self.codes
            .iter()
            .find(|(c, _)| c == code)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// Codes bound to an action (for the settings screen).
    pub fn codes_for(&self, a: Action) -> Vec<&str> {
        self.codes
            .iter()
            .filter(|(_, v)| v.contains(&a))
            .map(|(c, _)| c.as_str())
            .collect()
    }

    /// Rebind: `action=Code,Code;action=Code` replaces those actions' keys.
    pub fn apply(&mut self, spec: &str) {
        for kv in spec.split(';') {
            let Some((a, codes)) = kv.split_once('=') else {
                continue;
            };
            let Some(act) = Action::from_id(a.trim()) else {
                continue;
            };
            for (_, v) in self.codes.iter_mut() {
                v.retain(|x| *x != act);
            }
            for c in codes.split(',').map(str::trim).filter(|c| !c.is_empty()) {
                self.add(c, act);
            }
        }
        self.codes.retain(|(_, v)| !v.is_empty());
    }

    /// `action=Code,Code;...` for every action (sent to the settings UI).
    pub fn to_spec(&self) -> String {
        ACTIONS
            .iter()
            .map(|a| format!("{}={}", a.id(), self.codes_for(*a).join(",")))
            .collect::<Vec<_>>()
            .join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_rebinding() {
        let mut b = Bindings::default();
        assert_eq!(b.actions("KeyE")[0], Action::Interact);
        assert!(b.actions("KeyE").contains(&Action::Ult));
        assert!(b.actions("KeyA").contains(&Action::Left));
        b.apply("dash=KeyC;bogus=KeyV");
        assert!(!b.actions("Space").contains(&Action::Dash));
        assert!(b.actions("Space").contains(&Action::Confirm));
        assert_eq!(b.actions("KeyC"), &[Action::Dash]);
        let spec = b.to_spec();
        let mut c = Bindings::default();
        c.apply(&spec);
        assert_eq!(c.actions("KeyC"), &[Action::Dash]);
    }
}

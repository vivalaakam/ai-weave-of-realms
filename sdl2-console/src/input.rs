use game::input::InputEvent;
use sdl2::keyboard::{Keycode, Mod};

/// Map an SDL2 keyboard event to a game `InputEvent`.
///
/// # Arguments
/// * `keycode` — SDL2 key code.
/// * `keymod` — Modifier keys (Ctrl, Shift, etc.).
///
/// # Returns
/// An `InputEvent` suitable for `EmbeddedApp::handle_input`.
pub fn map_key_event(keycode: Keycode, keymod: Mod) -> InputEvent {
    match keycode {
        Keycode::Space => InputEvent::NextTurn,
        Keycode::Escape => InputEvent::Back,
        Keycode::Up => InputEvent::Up,
        Keycode::Down => InputEvent::Down,
        Keycode::Left => InputEvent::Left,
        Keycode::Right => InputEvent::Right,
        Keycode::Tab => InputEvent::NextHero,
        Keycode::Return => InputEvent::Enter,
        Keycode::A if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::PanLeft,
        Keycode::D if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::PanRight,
        Keycode::W if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::PanUp,
        Keycode::S if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::PanDown,
        Keycode::H => InputEvent::CursorLeft,
        Keycode::J => InputEvent::CursorDown,
        Keycode::K => InputEvent::CursorUp,
        Keycode::L => InputEvent::CursorRight,
        Keycode::B if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('b'),
        Keycode::C if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('c'),
        Keycode::E if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('e'),
        Keycode::F if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('f'),
        Keycode::G if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('g'),
        Keycode::I if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('i'),
        Keycode::M if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('m'),
        Keycode::N if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('n'),
        Keycode::O if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('o'),
        Keycode::P if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('p'),
        Keycode::Q if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('q'),
        Keycode::R if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('r'),
        Keycode::T if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('t'),
        Keycode::U if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('u'),
        Keycode::V if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('v'),
        Keycode::X if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('x'),
        Keycode::Y if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('y'),
        Keycode::Z if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('z'),
        Keycode::Num0 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('0'),
        Keycode::Num1 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('1'),
        Keycode::Num2 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('2'),
        Keycode::Num3 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('3'),
        Keycode::Num4 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('4'),
        Keycode::Num5 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('5'),
        Keycode::Num6 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('6'),
        Keycode::Num7 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('7'),
        Keycode::Num8 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('8'),
        Keycode::Num9 if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('9'),
        Keycode::Minus if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => InputEvent::Key('-'),
        Keycode::Underscore if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
            InputEvent::Key('_')
        }
        Keycode::Period if !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
            InputEvent::Key('.')
        }
        Keycode::F11 => InputEvent::None,
        _ => InputEvent::None,
    }
}

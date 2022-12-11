use core::ops::Index;

use bitflags::bitflags;
use lazy_static::lazy_static;
use spin::Mutex;

use crate::serial::port_read_byte;

mod dvorak;
mod keys;

pub use keys::Key;

const PS2_KEYBOARD_PORT: u16 = 0x60;

bitflags! {
    pub struct KeyboardModifiers: u8 {
        const CONTROL = 1;
        const SHIFT = 1 << 1;
        const OPTION = 1 << 2;
        const META = 1 << 3;
    }
}

pub trait KeycodeMap: Index<u8, Output = Key> {
    fn modifiers(&self, keycode: u8) -> KeyboardModifiers;
}

pub struct KeyboardState<'a> {
    port: u16,
    modifiers: KeyboardModifiers,
    keymap: &'a dyn KeycodeMap,
}

fn modifier(key: Key) -> KeyboardModifiers {
    match key {
        Key::LeftControl | Key::RightControl => KeyboardModifiers::CONTROL,
        Key::LeftShift | Key::RightShift => KeyboardModifiers::SHIFT,
        Key::LeftOption | Key::RightOption => KeyboardModifiers::OPTION,
        Key::LeftMeta | Key::RightMeta => KeyboardModifiers::META,
        _ => KeyboardModifiers::empty(),
    }
}

// TODO: we'll rethink the API once we have async/await as an event subscription
impl<'a> KeyboardState<'a> {
    pub fn new(port: u16, keymap: &'a dyn KeycodeMap) -> KeyboardState<'a> {
        KeyboardState {
            port,
            keymap,
            modifiers: KeyboardModifiers::empty(),
        }
    }
    pub fn read_scancode(&mut self) -> Option<(Key, KeyboardModifiers)> {
        // Shouldn't ever be unsafe to read, but might be junky.
        // If that's not true, move unsafety to caller.
        let scancode = unsafe { port_read_byte(self.port) };
        // Top bit is 1 for released, 0 for pressed, rest are keycode
        let released = (scancode >> 7) != 0;
        let keycode = scancode & 0x7F;
        let key = self.keymap[keycode];
        let modifier = modifier(key);
        if !modifier.is_empty() {
            self.modifiers.set(modifier, !released);
        }
        match released {
            true => Some((key, self.modifiers)),
            false => None,
        }
    }
}

// spin::Mutex implements Send and Sync for any Send types,
// so we need our KeyboardState to be Send.
//
// I don't actually know why the compiler doesn't infer this.
// Send should _not_ be safe for refs with arbitrary lifetimes,
// but _should_ always be safe for &'static.
unsafe impl Send for KeyboardState<'static> {}

lazy_static! {
    pub static ref KEYBOARD: Mutex<KeyboardState<'static>> =
        Mutex::new(KeyboardState::new(PS2_KEYBOARD_PORT, &dvorak::MAP));
}

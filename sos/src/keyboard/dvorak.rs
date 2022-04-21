use core::ops::Index;

use super::{Key, KeyboardModifiers, KeycodeMap};

pub struct Dvorak([Key; 128]);

impl KeycodeMap for Dvorak {
    fn modifiers(&self, _keycode: u8) -> KeyboardModifiers {
        KeyboardModifiers::empty()
    }
}

impl Index<u8> for Dvorak {
    type Output = Key;
    fn index(&self, index: u8) -> &Key {
        return &self.0[index as usize];
    }
}

// pub static MAP: Dvorak = Dvorak(['\0'; 128]);
pub static MAP: Dvorak = Dvorak([
    Key::NotBound, // unknown
    Key::Escape,
    Key::Character('1', '!'),
    Key::Character('2', '@'),
    Key::Character('3', '#'),
    Key::Character('4', '$'),
    Key::Character('5', '%'),
    Key::Character('6', '^'),
    Key::Character('7', '&'),
    Key::Character('8', '*'),
    Key::Character('9', '('), // scancode = 10
    Key::Character('0', ')'),
    Key::Character('[', '{'),
    Key::Character(']', '}'),
    Key::Backspace,
    Key::Character('\t', '\t'),
    Key::Character('\'', '"'),
    Key::Character(',', '<'),
    Key::Character('.', '>'),
    Key::Character('p', 'P'),
    Key::Character('y', 'Y'), // scancode = 20
    Key::Character('f', 'F'),
    Key::Character('g', 'G'),
    Key::Character('c', 'C'),
    Key::Character('r', 'R'),
    Key::Character('l', 'L'),
    Key::Character('/', '?'),
    Key::Character('=', '+'),
    Key::Character('\n', '\n'),
    Key::LeftControl,
    Key::Character('a', 'A'), // scancode = 30
    Key::Character('o', 'O'),
    Key::Character('e', 'E'),
    Key::Character('u', 'U'),
    Key::Character('i', 'I'),
    Key::Character('d', 'D'),
    Key::Character('h', 'H'),
    Key::Character('t', 'T'),
    Key::Character('n', 'N'),
    Key::Character('s', 'S'),
    Key::Character('-', '_'), // scancode = 40
    Key::Character('`', '~'),
    Key::LeftShift,
    Key::Character('\\', '|'),
    Key::Character(';', ':'),
    Key::Character('q', 'Q'),
    Key::Character('j', 'J'),
    Key::Character('k', 'K'),
    Key::Character('x', 'X'),
    Key::Character('b', 'B'),
    Key::Character('m', 'M'), // scancode = 50
    Key::Character('w', 'W'),
    Key::Character('v', 'V'),
    Key::Character('z', 'Z'),
    Key::RightShift,
    Key::LeftMeta,
    Key::LeftOption,
    Key::Character(' ', ' '),
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 60
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 70
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 80
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 90
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 100
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 110
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound, // scancode = 120
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
    Key::NotBound,
]);

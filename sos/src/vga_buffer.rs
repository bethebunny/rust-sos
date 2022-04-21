use core::fmt;

use lazy_static::lazy_static;
use spin::Mutex;

const VGA_MEM_LOCATION: usize = 0xb8000;
const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer::new());
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        // Static lock, so avoid deadlocks where interrupt handlers try to aquire lock
        // by disabling interrupts.
        $crate::without_interrupt! {{
            use core::fmt::Write;
            $crate::vga_buffer::WRITER.lock().write_fmt(format_args!($($arg)*)).unwrap();
        }}
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)] // byte representation will be u8
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)] // bytes are laid out in order instead of undefined
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

type ScreenBuffer = [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT];

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut ScreenBuffer,
}

impl Writer {
    pub fn new() -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(Color::Yellow, Color::Black),
            buffer: unsafe { &mut *(VGA_MEM_LOCATION as *mut ScreenBuffer) },
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                self.buffer[BUFFER_HEIGHT - 1][self.column_position] = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                };
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        // Can't use copy_from_slice to copy from a vector to itself because of borrow checker
        // self.buffer.chars[..BUFFER_HEIGHT-1].copy_from_slice(&self.buffer.chars[1..])
        self.buffer.copy_within(1.., 0);
        self.clear_line(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_line(&mut self, line: usize) {
        let empty_line: [ScreenChar; BUFFER_WIDTH] = [ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        }; BUFFER_WIDTH];
        self.buffer[line].copy_from_slice(&empty_line);
    }

    pub fn write_string(&mut self, s: &str) {
        s.bytes()
            .map(|c| match c {
                0x20..=0x7e | b'\n' => c,
                _ => 0xfe, // non-printable ASCII bytes
            })
            .for_each(|c| self.write_byte(c))
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

mod test {
    use super::*;

    #[test_case]
    fn test_println() {
        println!("hello");
    }

    #[test_case]
    fn test_println_many() {
        (0..100).for_each(|i| println!("{}", i));
    }

    #[test_case]
    fn test_print_long() {
        (0..1000).for_each(|i| print!("{}", i));
    }

    #[allow(dead_code)] //#[test_case]  # TODO: test broken
    fn test_print_long_wraps_to_newline() {
        println!(); // clear line
        let line = WRITER.lock().buffer[BUFFER_HEIGHT - 1];
        let last_char = char::from(b'a' + BUFFER_WIDTH as u8);
        ('a'..last_char).for_each(|i| print!("{}", i));
        for i in 0..BUFFER_WIDTH {
            assert_ne!(line[i].ascii_character, b' ');
        }
        println!(); // clear line
        let one_more = char::from((last_char as u8) + 1);
        ('a'..one_more).for_each(|i| print!("{}", i));
        assert_eq!(line[0].ascii_character, one_more as u8);
        for i in 1..BUFFER_WIDTH {
            assert_eq!(line[i].ascii_character, b' ');
        }
    }

    // TODO: test newline moves previous lines up
    // TODO: test color codes
    // TODO: test unprintable characters

    #[test_case]
    fn test_print_output() {
        println!(); // reset column position
        let test_str = "printed the thing";
        print!("{}", test_str);
        let line = WRITER.lock().buffer[BUFFER_HEIGHT - 1];
        for (i, byte) in test_str.bytes().enumerate() {
            assert_eq!(line[i].ascii_character, byte);
        }
        for i in test_str.len()..BUFFER_WIDTH {
            assert_eq!(line[i].ascii_character, b' ');
        }
    }
}

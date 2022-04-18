use core::arch::asm;
use core::fmt;

use bitflags::bitflags;
use lazy_static::lazy_static;
use spin::Mutex;

const SERIAL1_PORT: u16 = 0x3F8;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let serial_port = SerialPort::new(SERIAL1_PORT);
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        $crate::serial::SERIAL1.lock().write_fmt(format_args!($($arg)*)).unwrap();
    })
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}

#[cfg(target_arch = "x86_64")]
pub unsafe fn port_write_byte(port: u16, byte: u8) {
    // Rust inline asm reference: https://doc.rust-lang.org/nightly/reference/inline-assembly.html
    // OUT instruction reference: https://www.felixcloutier.com/x86/out
    asm!("out dx, al", in("dx") port, in("al") byte);
}

#[cfg(target_arch = "x86_64")]
pub unsafe fn port_read_byte(port: u16) -> u8 {
    let mut byte: u8;
    asm!("in al, dx", in("dx") port, out("al") byte);
    byte
}

bitflags! {
    struct LineStatus: u8 {
        const INPUT_FULL = 1;
        const OUTPUT_EMPTY = 1 << 5;
    }
}

pub struct SerialPort {
    data_port: u16,
}

impl SerialPort {
    pub fn new(data_port: u16) -> SerialPort {
        SerialPort { data_port }
    }

    pub fn init(&self) {
        let interrupt_enable = self.data_port + 1;
        let fifo_ctrl = self.data_port + 2;
        let line_ctrl = self.data_port + 3;
        let modem_ctrl = self.data_port + 4;

        unsafe {
            // Taken from https://github.com/rust-osdev/uart_16550/blob/master/src/port.rs
            port_write_byte(interrupt_enable, 0x00); // Disable interrupts
            port_write_byte(line_ctrl, 0x80); // Enable DLAB, TODO docs

            // Set maximum speed to 38400 bps by configuring DLL and DLM
            port_write_byte(self.data_port, 0x03);
            port_write_byte(interrupt_enable, 0x00);

            // Disable DLAB and set data word length to 8 bits
            port_write_byte(line_ctrl, 0x03);

            // Enable FIFO, clear TX/RX queues and set interrupt watermark at 14 bytes
            port_write_byte(fifo_ctrl, 0xC7);

            // Mark data terminal ready, signal request to send
            // and enable auxilliary output #2 (used as interrupt line for CPU)
            port_write_byte(modem_ctrl, 0x08);
            port_write_byte(interrupt_enable, 0x00); // Enable interrupts
        }
    }

    unsafe fn line_status(&self) -> LineStatus {
        let line_status_port = self.data_port + 5;
        LineStatus::from_bits_truncate(port_read_byte(line_status_port))
    }

    unsafe fn wait_for_output_empty(&self) {
        while !self.line_status().contains(LineStatus::OUTPUT_EMPTY) {
            core::hint::spin_loop();
        }
    }

    unsafe fn wait_for_input_fill(&self) {
        while !self.line_status().contains(LineStatus::INPUT_FULL) {
            core::hint::spin_loop();
        }
    }

    // TODO: async implementations (one day :>)
    pub fn write_byte_raw(&self, byte: u8) {
        unsafe {
            self.wait_for_output_empty();
            port_write_byte(self.data_port, byte);
        }
    }

    pub fn write_byte(&self, byte: u8) {
        match byte {
            8 | 0x7f => {
                // TODO: docs
                self.write_byte_raw(8);
                self.write_byte_raw(b' ');
                self.write_byte_raw(8);
            }
            _ => self.write_byte_raw(byte),
        }
    }

    #[allow(dead_code)]
    pub fn read_byte(&self) -> u8 {
        unsafe {
            self.wait_for_input_fill();
            port_read_byte(self.data_port)
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.bytes().for_each(|byte| self.write_byte(byte));
        Ok(())
    }
}

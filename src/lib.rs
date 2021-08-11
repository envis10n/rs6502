use std::sync::{Arc, RwLock, Mutex};
use enumflags2::{BitFlags, bitflags};

pub mod error;
pub mod memory;
pub mod bus;
pub mod ops;
pub mod interrupts;

use bus::Bus;
use memory::Memory;
use interrupts::Interrupt;

pub type Result<T> = std::result::Result<T, error::Error>;

#[bitflags(default = UNK | Interrupt)]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CpuFlags {
    Carry = 0b00000001,
    Zero = 0b00000010,
    Interrupt = 0b00000100,
    Decimal = 0b00001000,
    Break = 0b00010000,
    UNK = 0b00100000,
    Overflow = 0b01000000,
    Negative = 0b10000000
}

pub struct Mos6502<'a> {
    pub status: BitFlags<CpuFlags>,
    pub accumulator: u8,
    pub index_x: u8,
    pub index_y: u8,
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub interrupt: Option<Interrupt>,
    pub bus: Arc<RwLock<Bus<'a>>>,
}

impl<'a> Memory for Mos6502<'a> {
    fn mem_read(&self, addr: u16) -> u8 {
        let reader = self.bus.read().unwrap();
        (*reader).mem_read(addr)
    }
    fn mem_write(&mut self, addr: u16, data: u8) {
        let mut writer = self.bus.write().unwrap();
        (*writer).mem_write(addr, data);
    }
}

impl<'a> Mos6502<'a> {
    pub fn new(bus: Arc<RwLock<Bus<'a>>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Mos6502 {
            status: BitFlags::default(),
            accumulator: 0,
            index_x: 0,
            index_y: 0,
            program_counter: 0,
            stack_pointer: 0,
            interrupt: None,
            bus: bus.clone()
        }))
    }
    fn _reset(&mut self) {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_error() {
        use super::error::Error;
        let e = Error::UnknownOpCode(0xff);
        panic!("{}", e);
    }
}

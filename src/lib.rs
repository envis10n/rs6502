use std::sync::{Arc, RwLock, Mutex};

#[macro_use]
pub mod error;
pub mod memory;
pub mod bus;

use bus::Bus;
use memory::Memory;

pub type Result<T> = std::result::Result<T, error::Error>;

pub struct Mos6502<'a> {
    bus: Arc<RwLock<Bus<'a>>>,
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
        Arc::new(Mutex::new(Mos6502 { bus: bus.clone() }))
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

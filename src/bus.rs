use crate::memory::Memory;
use crate::Result;
use crate::error::Error;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

pub struct Bus<'a> {
    memory_map: HashMap<(u16, u16), Arc<RwLock<dyn Memory + 'a>>>
}

impl<'a> Memory for Bus<'a> {
    fn mem_read(&self, addr: u16) -> u8 {
        if let Some(region) = self.get_region_place(addr) {
            if let Some(controller) = self.memory_map.get(&region) {
                let reader = controller.read().unwrap();
                (*reader).mem_read(addr)
            } else {
                0
            }
        } else {
            0
        }
    }
    fn mem_write(&mut self, addr: u16, data: u8) {
        if let Some(region) = self.get_region_place(addr) {
            if let Some(controller) = self.memory_map.get(&region) {
                let mut writer = controller.write().unwrap();
                (*writer).mem_write(addr, data);
            }
        }
    }
}

impl<'a> Bus<'a> {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Bus {
            memory_map: HashMap::new(),
        }))
    }
    fn get_region_place(&self, addr: u16) -> Option<(u16, u16)> {
        let mut result = None;
        for key in self.memory_map.keys() {
            if addr >= key.0 && addr <= key.1 {
                result = Some(key.clone());
                break;
            }
        }
        result
    }
    fn get_region_key(&self, start: u16, end: u16) -> Option<(u16, u16)> {
        let mut result = None;
        for key in self.memory_map.keys() {
            if start >= key.0 && end <= key.1 {
                result = Some(key.clone());
                break;
            }
        }
        result
    }
    pub fn map_region<C>(&mut self, start: u16, end: u16, controller: Arc<RwLock<C>>) -> Result<()> where C: Memory + 'a {
        if let Some(key) = self.get_region_key(start, end) {
            Err(Error::MemoryMapConflict(key))
        } else {
            self.memory_map.insert((start, end), controller.clone());
            Ok(())
        }
    }
}
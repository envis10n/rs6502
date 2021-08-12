use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, SystemTime};
use enumflags2::{BitFlags, bitflags};

pub mod error;
pub mod memory;
pub mod bus;
pub mod ops;
pub mod interrupts;

use bus::Bus;
use memory::Memory;
use interrupts::Interrupt;
use ops::AddressingMode;

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
    pub interrupt: Option<Interrupt<'a>>,
    pub bus: Arc<RwLock<Bus<'a>>>,
    pub clock_frequency: f32,
    pub clock_speed: f32,
    pub game_tick_time: SystemTime,
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
            bus: bus.clone(),
            clock_frequency: 1789773f32,
            clock_speed: (1000f32 / 1789773f32) * 1000f32, // Clock frequency from hz to seconds.
            game_tick_time: SystemTime::now(),
        }))
    }
    pub fn get_tick_delta(&mut self) -> f32 {
        let old = self.game_tick_time.clone();
        self.game_tick_time = SystemTime::now();
        self.game_tick_time.duration_since(old).unwrap().as_secs_f32()
    }
    fn handle_interrupt(&mut self) -> u8 {
        if let Some(interrupt) = self.interrupt.take() {
            let mut flags: BitFlags<CpuFlags> = self.status.clone();
            match interrupt {
                Interrupt::NMI(handler_) => {
                    self.stack_push_u16(self.program_counter); // Store PC
                    self.stack_push(flags.bits());
                    if let Some(mut handler) = handler_ {
                        handler();
                    }
                    self.program_counter = self.mem_read_u16(0xFFFA);
                    self.status.insert(CpuFlags::Interrupt);
                    7
                }
                Interrupt::BRK(handler_) => {
                    self.stack_push_u16(self.program_counter);
                    flags.insert(CpuFlags::Break);
                    self.stack_push(flags.bits());
                    if let Some(mut handler) = handler_ {
                        handler();
                    }
                    self.program_counter = self.mem_read_u16(0xFFFE);
                    self.status.insert(CpuFlags::Interrupt);
                    6
                }
                Interrupt::IRQ(handler_) => {
                    self.stack_push_u16(self.program_counter);
                    self.stack_push(flags.bits());
                    if let Some(mut handler) = handler_ {
                        handler();
                    }
                    self.program_counter = self.mem_read_u16(0xFFFE);
                    self.status.insert(CpuFlags::Interrupt);
                    6
                }
                Interrupt::RESET => {
                    self.reset();
                    8
                }
            }
        } else {
            0
        }
    }
    fn get_operand_address(&mut self, mode: AddressingMode) -> (u16, bool) {
        const fn page_crossed(addr1: u16, addr2: u16) -> bool {
            (addr1 & 0xFF00) != (addr2 & 0xFF00)
        }
        match mode {
            AddressingMode::Immediate => (self.program_counter, false),
            AddressingMode::Absolute => (self.mem_read_u16(self.program_counter), false),
            AddressingMode::ZeroPage => (self.mem_read(self.program_counter) as u16, false),
            AddressingMode::Indirect => {
                let addr = self.mem_read_u16(self.program_counter);
                (self.mem_read_u16(addr), false)
            }
            AddressingMode::AbsoluteX => {
                let absolute_addr = self.mem_read_u16(self.program_counter);
                let effective_addr = absolute_addr.wrapping_add(self.index_x as u16);
                (effective_addr, page_crossed(absolute_addr, effective_addr))
            }
            AddressingMode::AbsoluteY => {
                let absolute_addr = self.mem_read_u16(self.program_counter);
                let effective_addr = absolute_addr.wrapping_add(self.index_y as u16);
                (effective_addr, page_crossed(absolute_addr, effective_addr))
            }
            AddressingMode::ZeroPageX => {
                let addr = self.mem_read(self.program_counter);
                (addr.wrapping_add(self.index_x) as u16, false)
            }
            AddressingMode::ZeroPageY => {
                let addr = self.mem_read(self.program_counter);
                (addr.wrapping_add(self.index_y) as u16, false)
            }
            AddressingMode::IndirectX => {
                let addr = self.mem_read(self.program_counter);
                let zpg = addr.wrapping_add(self.index_x) as u16;
                (self.mem_read_u16_wrap(zpg), false)
            }
            AddressingMode::IndirectY => {
                let addr = self.mem_read(self.program_counter);
                let zpg = self.mem_read_u16_wrap(addr as u16);
                let addr = zpg + self.index_y as u16;
                (addr, page_crossed(zpg, addr))
            }
            AddressingMode::Relative => {
                let offset = self.mem_read(self.program_counter);
                let mut addr = self.program_counter + 1 + offset as u16;
                if offset >= 0x80 {
                    addr -= 0x100;
                }
                (addr, false)
            }
            _ => (0, false)
        }
    }
    fn execute(&mut self) {
        let tick_delta = self.get_tick_delta();
        let mut stall_cycles: u32 = 0;
        let mut cpu_cycles: u32 = (tick_delta * self.clock_speed * self.clock_frequency) as u32;
        loop {
            if stall_cycles > 0 {
                stall_cycles -= 1;
                continue;
            }
            if cpu_cycles == 0 {
                break;
            }
            let interrupt_cycles = self.handle_interrupt();
            // Check if it would wrap
            if interrupt_cycles as u32 > cpu_cycles {
                stall_cycles = interrupt_cycles as u32 - cpu_cycles;
                cpu_cycles = 0;
            } else {
                cpu_cycles -= interrupt_cycles as u32;
            }
            if interrupt_cycles > 0 {
                continue;
            }
            let instruction = self.mem_read(self.program_counter);
            self.program_counter += 1;
            // TODO: Stuff
        }
    }
    fn reset(&mut self) {
        self.stack_pointer = 0xFD;
        self.status = BitFlags::default();
        self.index_x = 0;
        self.index_y = 0;
        self.accumulator = 0;
        self.program_counter = self.mem_read_u16(0xFFFC);
    }
    /// Decrement the stack pointer and read the value stored.
    fn stack_pop(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
        let addr = u16::from_le_bytes([self.stack_pointer, 0x01]);
        self.mem_read(addr)
    }
    /// Push data to the stack and increment the pointer.
    fn stack_push(&mut self, data: u8) {
        let addr = u16::from_le_bytes([self.stack_pointer, 0x01]);
        self.mem_write(addr, data);
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
    }
    /// Push a 16-bit value to the stack
    fn stack_push_u16(&mut self, data: u16) {
        let bytes = u16::to_le_bytes(data);
        self.stack_push(bytes[0]);
        self.stack_push(bytes[1]);
    }
    /// Pop a 16-bit value from the stack
    fn stack_pop_u16(&mut self) -> u16 {
        let hi = self.stack_pop();
        let lo = self.stack_pop();
        u16::from_le_bytes([lo, hi])
    }
    /// Sets the Carry flag
    fn sec(&mut self) {
        self.status.insert(CpuFlags::Carry);
    }
    /// Clears the Carry flag
    fn clc(&mut self) {
        self.status.remove(CpuFlags::Carry);
    }
    fn update_zero_negative_flags(&mut self, value: u8) {
        if value == 0 {
            self.status.insert(CpuFlags::Zero);
        } else {
            self.status.remove(CpuFlags::Zero);
        }
        // If bit 7 is set to 1, set the negative flag
        if (value & 0b10000000) != 0 {
            self.status.insert(CpuFlags::Negative);
        } else {
            self.status.remove(CpuFlags::Negative);
        }
    }
    /// Set the accumulator
    fn add_to_accumulator(&mut self, value: u8, with_carry: bool) {
        let value: u8 = value + if self.status.contains(CpuFlags::Carry) { 1 } else { 0 };
        let temp: u16 = self.accumulator as u16 + value as u16;
        // If the value would wrap
        if temp > 255 && with_carry {
            self.sec();
        } else {
            self.clc();
        }
        self.accumulator = self.accumulator.wrapping_add(value);
        self.update_zero_negative_flags(self.accumulator);
    }
    // MOS 6502 Instruction Handlers
    /// Add with Carry
    fn adc(&mut self, mode: AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.add_to_accumulator(self.mem_read(addr), true);
        page_cross
    }
    /// Logical AND
    fn and(&mut self, mode: AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.accumulator = self.accumulator & self.mem_read(addr);
        self.update_zero_negative_flags(self.accumulator);
        page_cross
    }
    /// Arithmetic Shift Left
    fn asl(&mut self, mode: AddressingMode) -> bool {
        let (addr, _) = self.get_operand_address(mode);
        let mut data: u8 = self.accumulator;
        if addr != 0 {
            data = self.mem_read(addr);
        }
        if data >> 7 == 1 {
            self.sec();
        } else {
            self.clc();
        }
        data = data << 1;
        if addr == 0 {
            self.accumulator = data;
        } else {
            self.mem_write(addr, data);
        }
        self.update_zero_negative_flags(data);
        false
    }
}

#[cfg(test)]
mod tests {
    struct RAMChip {
        memory: [u8; 0xFFFF]
    }
    impl RAMChip {
        pub fn new() -> Arc<RwLock<Self>> {
            Arc::new(RwLock::new(RAMChip { memory: [0; 0xFFFF] }))
        }
    }
    impl Memory for RAMChip {
        fn mem_read(&self, addr: u16) -> u8 {
            self.memory[addr as usize]
        }
        fn mem_write(&mut self, addr: u16, data: u8) {
            self.memory[addr as usize] = data;
        }
    }
    use crate::bus::Bus;
    use crate::memory::Memory;
    use super::*;
    #[test]
    fn test_signed() {
        let bus = Bus::new();
        let ram = RAMChip::new();
        {
            let mut block = bus.write().unwrap();
            (*block).map_region(0, 0xFFFF, ram.clone()).unwrap(); // Add ram chip
            (*block).mem_write_signed(0x01ff, -45);
            (*block).mem_write_signed(0x01fe, 56);
            (*block).mem_write(0x01fd, 224);
        }
        // Read values
        {
            let block = bus.read().unwrap();
            let signed_a = (*block).mem_read_signed(0x01ff);
            let signed_b = (*block).mem_read_signed(0x01fe);
            let unsigned_c = (*block).mem_read(0x01fd);
            let signed_c = (*block).mem_read_signed(0x01fd);
            assert_eq!(signed_a, -45);
            assert_eq!(signed_b, 56);
            assert_eq!(unsigned_c, 224);
            assert_eq!(signed_c, -32);
        }
    }
}

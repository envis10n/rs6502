use enumflags2::{bitflags, BitFlags};
use std::collections::HashMap;
use std::ops::Add;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

#[macro_use]
extern crate lazy_static;

pub mod bus;
pub mod error;
pub mod interrupts;
pub mod memory;
pub mod ops;
pub mod trace;

use bus::Bus;
use interrupts::Interrupt;
use memory::Memory;
use ops::{AddressingMode, OpCode};

#[cfg(feature = "trace")]
use trace::Trace;

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
    Negative = 0b10000000,
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

#[cfg(feature = "trace")]
impl<'a> Trace for Arc<Mutex<Mos6502<'a>>> {
    fn trace(&self) -> String {
        let mut locker = self.lock().unwrap();
        let cpu = &mut *locker;
        cpu.trace()
    }
}

#[cfg(feature = "trace")]
impl<'a> Trace for Mos6502<'a> {
    fn trace(&self) -> String {
        let cpu = self;

        let ref opcodes: HashMap<u8, &'static OpCode> = *ops::OPCODES_MAP;

        let code = self.mem_read(cpu.program_counter);
        let ops = opcodes.get(&code).unwrap();

        let begin = cpu.program_counter;
        let mut hex_dump = vec![];
        hex_dump.push(code);

        let (mem_addr, stored_value) = match ops.mode {
            AddressingMode::Immediate | AddressingMode::NoneAddressing => (0, 0),
            _ => {
                let (addr, _) = cpu.get_absolute_address(&ops.mode, begin + 1);
                (addr, cpu.mem_read(addr))
            }
        };

        let tmp = match ops.len {
            1 => match ops.code {
                0x0a | 0x4a | 0x2a | 0x6a => format!("A "),
                _ => String::from(""),
            },
            2 => {
                let address = cpu.mem_read(begin + 1);
                hex_dump.push(address);

                match ops.mode {
                    AddressingMode::Immediate => format!("#${:02x}", address),
                    AddressingMode::ZeroPage => format!("${:02x} = {:02x}", mem_addr, stored_value),
                    AddressingMode::ZeroPageX => format!(
                        "${:02x},X @ {:02x} = {:02x}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::ZeroPageY => format!(
                        "${:02x},Y @ {:02x} = {:02x}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::IndirectX => format!(
                        "(${:02x},X) @ {:02x} = {:04x} = {:02x}",
                        address,
                        (address.wrapping_add(cpu.index_x)),
                        self.mem_read_u16(address.wrapping_add(cpu.index_x) as u16),
                        self.mem_read(self.mem_read_u16(address.wrapping_add(cpu.index_x) as u16))
                    ),
                    AddressingMode::IndirectY => format!(
                        "(${:02x}),Y = {:04x} @ {:04x} = {:02x}",
                        address,
                        (mem_addr.wrapping_sub(cpu.index_y as u16)),
                        mem_addr,
                        stored_value,
                    ),
                    AddressingMode::NoneAddressing => {
                        let address: usize =
                            (begin as usize + 2).wrapping_add((address as i8) as usize);
                        format!("${:04x}", address)
                    }
                    _ => panic!(
                        "Unexpected addressing mode {:?} has ops len of 2. code {:02x}",
                        ops.mode, ops.code
                    ),
                }
            }
            3 => {
                let address_lo = cpu.mem_read(begin + 1);
                let address_hi = cpu.mem_read(begin + 2);
                hex_dump.push(address_lo);
                hex_dump.push(address_hi);

                let address = cpu.mem_read_u16(begin + 1);

                match ops.mode {
                    AddressingMode::Absolute => format!("${:04x} = {:02x}", mem_addr, stored_value),
                    AddressingMode::AbsoluteX => format!(
                        "${:04x},X @ {:04x} = {:02x}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::AbsoluteY => format!(
                        "${:04x},Y @ {:04x} = {:02x}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::NoneAddressing => {
                        if ops.code == 0x6c {
                            let jmp_addr = if address & 0x00ff == 0x00ff {
                                let lo = cpu.mem_read(address);
                                let hi = cpu.mem_read(address & 0xff00);
                                (hi as u16) << 8 | (lo as u16)
                            } else {
                                cpu.mem_read_u16(address)
                            };

                            format!("(${:04x}) = {:04x}", address, jmp_addr)
                        } else {
                            format!("${:04x}", address)
                        }
                    }
                    _ => panic!(
                        "Unexpected addressing mode {:?} has ops len 3. code {:02x}",
                        ops.mode, ops.code
                    ),
                }
            }
            _ => String::from(""),
        };

        let hex_str = hex_dump
            .iter()
            .map(|z| format!("{:02x}", z))
            .collect::<Vec<String>>()
            .join(" ");
        let asm_str = format!("{:04x}  {:8} {: >4} {}", begin, hex_str, ops.mnemonic, tmp)
            .trim()
            .to_string();

        format!(
            "{:47} A:{:02x} X:{:02x} Y:{:02x} P:{:08b} SP:{:02x} | 02h: {:02x} 03h: {:02x}",
            asm_str, cpu.accumulator, cpu.index_x, cpu.index_y, cpu.status, cpu.stack_pointer, cpu.mem_read(0x0002), cpu.mem_read(0x0003)
        )
        .to_ascii_uppercase()
    }
}

impl<'a> Memory for Arc<Mutex<Mos6502<'a>>> {
    fn mem_read(&self, addr: u16) -> u8 {
        let reader = self.lock().unwrap();
        (*reader).mem_read(addr)
    }
    fn mem_write(&mut self, addr: u16, data: u8) {
        let mut writer = self.lock().unwrap();
        (*writer).mem_write(addr, data);
    }
}

impl<'a> Memory for Mos6502<'a> {
    fn mem_read(&self, addr: u16) -> u8 {
        self.bus.mem_read(addr)
    }
    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.mem_write(addr, data);
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
        self.game_tick_time
            .duration_since(old)
            .unwrap()
            .as_secs_f32()
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
                    7
                }
                Interrupt::IRQ(handler_) => {
                    self.stack_push_u16(self.program_counter);
                    self.stack_push(flags.bits());
                    if let Some(mut handler) = handler_ {
                        handler();
                    }
                    self.program_counter = self.mem_read_u16(0xFFFE);
                    self.status.insert(CpuFlags::Interrupt);
                    7
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
    fn get_absolute_address(&self, mode: &AddressingMode, addr: u16) -> (u16, bool) {
        const fn page_crossed(addr1: u16, addr2: u16) -> bool {
            (addr1 & 0xFF00) != (addr2 & 0xFF00)
        }
        match mode {
            AddressingMode::Immediate => (addr, false),
            AddressingMode::Absolute => (self.mem_read_u16(addr), false),
            AddressingMode::ZeroPage => (self.mem_read(addr) as u16, false),
            AddressingMode::AbsoluteX => {
                let absolute_addr = self.mem_read_u16(addr);
                let effective_addr = absolute_addr.wrapping_add(self.index_x as u16);
                (effective_addr, page_crossed(absolute_addr, effective_addr))
            }
            AddressingMode::AbsoluteY => {
                let absolute_addr = self.mem_read_u16(addr);
                let effective_addr = absolute_addr.wrapping_add(self.index_y as u16);
                (effective_addr, page_crossed(absolute_addr, effective_addr))
            }
            AddressingMode::ZeroPageX => {
                let addr = self.mem_read(addr);
                (addr.wrapping_add(self.index_x) as u16, false)
            }
            AddressingMode::ZeroPageY => {
                let addr = self.mem_read(addr);
                (addr.wrapping_add(self.index_y) as u16, false)
            }
            AddressingMode::IndirectX => {
                let base = self.mem_read(addr) as u16;
                let ptr = (base as u8).wrapping_add(self.index_x);
                let mem_addr = self.mem_read_u16_wrap(ptr as u16 & 0x00FF);
                (mem_addr, false)
            }
            AddressingMode::IndirectY => {
                let base = self.mem_read(addr) as u16;
                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.index_y as u16);
                (deref, page_crossed(deref, deref_base))
            }
            AddressingMode::NoneAddressing => {
                (0x0000, false)
            }
            _ => panic!("mode {:?} is not supported", mode),
        }
    }
    fn get_operand_address(&self, mode: &AddressingMode) -> (u16, bool) {
        match mode {
            AddressingMode::Immediate => (self.program_counter, false),
            _ => self.get_absolute_address(mode, self.program_counter),
        }
    }
    pub fn execute(&mut self) -> Result<()> {
        let ref opcodes: HashMap<u8, &'static ops::OpCode> = *ops::OPCODES_MAP;
        let tick_delta = self.get_tick_delta();
        let mut stall_cycles: u32 = 0;
        let mut cpu_cycles: u32 = (tick_delta * self.clock_speed * self.clock_frequency) as u32;
        loop {
            if self.interrupt.is_some() {
                cpu_cycles = 0;
                stall_cycles = self.handle_interrupt() as u32;
                continue;
            }
            if stall_cycles > 0 {
                stall_cycles -= 1;
                continue;
            }
            if cpu_cycles == 0 {
                break Ok(());
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
            #[cfg(feature = "trace")]
            println!("{}", self.trace());

            let instruction = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;
            if let Some(opcode) = opcodes.get(&instruction) {
                let consumed_cycles: u8 = match opcode.mnemonic {
                    "BRK" => {
                        self.interrupt = Some(Interrupt::BRK(None));
                        continue;
                    }
                    "NOP" | "*NOP" => match opcode.code {
                        0x04 | 0x44 | 0x64 | 0x14 | 0x34 | 0x54 | 0x74 | 0xd4 | 0xf4 | 0x0c
                        | 0x1c | 0x3c | 0x5c | 0x7c | 0xdc | 0xfc => {
                            let (addr, page_cross) = self.get_operand_address(&opcode.mode);
                            let _data = self.mem_read(addr);
                            if page_cross {
                                opcode.cycles + 1
                            } else {
                                opcode.cycles
                            }
                        }
                        _ => opcode.cycles,
                    },
                    "ADC" => opcode.cycles + self.adc(&opcode.mode) as u8,
                    "SBC" => opcode.cycles + self.sbc(&opcode.mode) as u8,
                    "AND" => opcode.cycles + self.and(&opcode.mode) as u8,
                    "EOR" => opcode.cycles + self.eor(&opcode.mode) as u8,
                    "ORA" => opcode.cycles + self.ora(&opcode.mode) as u8,
                    "ASL" => {
                        self.asl(&opcode.mode);
                        opcode.cycles
                    }
                    "LSR" => {
                        self.lsr(&opcode.mode);
                        opcode.cycles
                    }
                    "ROL" => {
                        self.rol(&opcode.mode);
                        opcode.cycles
                    }
                    "ROR" => {
                        self.ror(&opcode.mode);
                        opcode.cycles
                    }
                    "INC" => {
                        self.inc(&opcode.mode);
                        opcode.cycles
                    }
                    "INX" => {
                        self.inx();
                        opcode.cycles
                    }
                    "INY" => {
                        self.iny();
                        opcode.cycles
                    }
                    "DEC" => {
                        self.dec(&opcode.mode);
                        opcode.cycles
                    }
                    "DEX" => {
                        self.dex();
                        opcode.cycles
                    }
                    "DEY" => {
                        self.dey();
                        opcode.cycles
                    }
                    "CMP" => opcode.cycles + self.compare(&opcode.mode, self.accumulator) as u8,
                    "CPY" => opcode.cycles + self.compare(&opcode.mode, self.index_y) as u8,
                    "CPX" => opcode.cycles + self.compare(&opcode.mode, self.index_x) as u8,
                    "JMP" => {
                        if opcode.code == 0x4c {
                            self.jmp();
                        } else {
                            // indirect
                            let mem_address = self.mem_read_u16(self.program_counter);

                            let indirect_ref = if mem_address & 0x00ff == 0x00ff {
                                let lo = self.mem_read(mem_address);
                                let hi = self.mem_read(mem_address & 0xff00);
                                (hi as u16) << 8 | (lo as u16)
                            } else {
                                self.mem_read_u16(mem_address)
                            };

                            self.program_counter = indirect_ref;
                        }
                        opcode.cycles
                    }
                    "JSR" => {
                        self.jsr();
                        opcode.cycles
                    }
                    "RTS" => {
                        self.rts();
                        opcode.cycles
                    }
                    "RTI" => {
                        self.rti();
                        opcode.cycles
                    }
                    "BNE" => opcode.cycles + self.bne(),
                    "BVS" => opcode.cycles + self.bvs(),
                    "BVC" => opcode.cycles + self.bvc(),
                    "BMI" => opcode.cycles + self.bmi(),
                    "BEQ" => opcode.cycles + self.beq(),
                    "BCS" => opcode.cycles + self.bcs(),
                    "BCC" => opcode.cycles + self.bcc(),
                    "BPL" => opcode.cycles + self.bpl(),
                    "BIT" => {
                        self.bit(&opcode.mode);
                        opcode.cycles
                    }
                    "LDA" => opcode.cycles + self.lda(&opcode.mode) as u8,
                    "LDX" => opcode.cycles + self.ldx(&opcode.mode) as u8,
                    "LDY" => opcode.cycles + self.ldy(&opcode.mode) as u8,
                    "STA" => {
                        self.sta(&opcode.mode);
                        opcode.cycles
                    }
                    "STX" => {
                        self.stx(&opcode.mode);
                        opcode.cycles
                    }
                    "STY" => {
                        self.sty(&opcode.mode);
                        opcode.cycles
                    }
                    "CLD" => {
                        self.status.remove(CpuFlags::Decimal);
                        opcode.cycles
                    }
                    "CLI" => {
                        self.status.remove(CpuFlags::Interrupt);
                        opcode.cycles
                    }
                    "CLV" => {
                        self.status.remove(CpuFlags::Overflow);
                        opcode.cycles
                    }
                    "CLC" => {
                        self.status.remove(CpuFlags::Carry);
                        opcode.cycles
                    }
                    "SEC" => {
                        self.status.insert(CpuFlags::Carry);
                        opcode.cycles
                    }
                    "SEI" => {
                        self.status.insert(CpuFlags::Interrupt);
                        opcode.cycles
                    }
                    "SED" => {
                        self.status.insert(CpuFlags::Decimal);
                        opcode.cycles
                    }
                    "TAX" => {
                        self.tax();
                        opcode.cycles
                    }
                    "TAY" => {
                        self.tay();
                        opcode.cycles
                    }
                    "TSX" => {
                        self.tsx();
                        opcode.cycles
                    }
                    "TXA" => {
                        self.txa();
                        opcode.cycles
                    }
                    "TXS" => {
                        self.txs();
                        opcode.cycles
                    }
                    "TYA" => {
                        self.tya();
                        opcode.cycles
                    }
                    "PHA" => {
                        self.pha();
                        opcode.cycles
                    }
                    "PLA" => {
                        self.pla();
                        opcode.cycles
                    }
                    "PHP" => {
                        self.php();
                        opcode.cycles
                    }
                    "PLP" => {
                        self.plp();
                        opcode.cycles
                    }
                    "*DCP" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let mut data = self.mem_read(addr);
                        data = data.wrapping_sub(1);
                        self.mem_write(addr, data);
                        if data <= self.accumulator {
                            self.status.insert(CpuFlags::Carry);
                        }
                        self.update_zero_negative_flags(self.accumulator.wrapping_sub(data));
                        opcode.cycles
                    }
                    "*RLA" => {
                        let data = self.rol(&opcode.mode);
                        self.accumulator = data & self.accumulator;
                        self.update_zero_negative_flags(self.accumulator);
                        opcode.cycles
                    }
                    "*SLO" => {
                        let data = self.asl(&opcode.mode);
                        self.accumulator = data | self.accumulator;
                        self.update_zero_negative_flags(self.accumulator);
                        opcode.cycles
                    }
                    "*SRE" => {
                        self.lsr(&opcode.mode);
                        self.eor(&opcode.mode);
                        opcode.cycles
                    }
                    "*AXS" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let data = self.mem_read(addr);
                        let x_and_a = self.index_x & self.accumulator;
                        let result = x_and_a.wrapping_sub(data);

                        if data <= x_and_a {
                            self.status.insert(CpuFlags::Carry);
                        }
                        self.update_zero_negative_flags(result);
                        self.index_x = result;
                        opcode.cycles
                    }
                    "*ARR" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let data = self.mem_read(addr);
                        self.accumulator = self.accumulator & data;
                        self.update_zero_negative_flags(self.accumulator);
                        self.ror(&opcode.mode);
                        opcode.cycles
                    }
                    "*SBC" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let data = self.mem_read(addr);
                        self.sub_from_accumulator(data);
                        opcode.cycles
                    }
                    "*ANC" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let data = self.mem_read(addr);
                        self.accumulator = data & self.accumulator;
                        self.update_zero_negative_flags(self.accumulator);
                        if self.status.contains(CpuFlags::Negative) {
                            self.status.insert(CpuFlags::Carry);
                        } else {
                            self.status.remove(CpuFlags::Carry);
                        }
                        opcode.cycles
                    }
                    "*ALR" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let data = self.mem_read(addr);
                        self.accumulator = data & self.accumulator;
                        self.update_zero_negative_flags(self.accumulator);
                        self.lsr(&opcode.mode);
                        opcode.cycles
                    }
                    "*RRA" => {
                        let data = self.ror(&opcode.mode);
                        self.add_to_accumulator(data);
                        opcode.cycles
                    }
                    "*ISC" => {
                        self.inc(&opcode.mode);
                        self.sbc(&opcode.mode);
                        opcode.cycles
                    }
                    "*LAX" => {
                        self.lda(&opcode.mode);
                        self.accumulator = self.index_x;
                        opcode.cycles
                    }
                    "*SAX" => {
                        let data = self.accumulator & self.index_x;
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        self.mem_write(addr, data);
                        opcode.cycles
                    }
                    "*LXA" => {
                        self.lda(&opcode.mode);
                        self.tax();
                        opcode.cycles
                    }
                    "*XAA" => {
                        self.accumulator = self.index_x;
                        self.update_zero_negative_flags(self.accumulator);
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let data = self.mem_read(addr);
                        self.accumulator = data & self.accumulator;
                        self.update_zero_negative_flags(self.accumulator);
                        opcode.cycles
                    }
                    "*LAS" => {
                        let (addr, _) = self.get_operand_address(&opcode.mode);
                        let mut data = self.mem_read(addr);
                        data = data & self.stack_pointer;
                        self.accumulator = data;
                        self.index_x = data;
                        self.stack_pointer = data;
                        self.update_zero_negative_flags(data);
                        opcode.cycles
                    }
                    "*TAS" => {
                        let data = self.accumulator & self.index_x;
                        self.stack_pointer = data;
                        let mem_address =
                            self.mem_read_u16(self.program_counter) + self.index_y as u16;
                        let data = ((mem_address >> 8) as u8 + 1) & self.stack_pointer;
                        self.mem_write(mem_address, data);
                        opcode.cycles
                    }
                    "*AHX" => {
                        match opcode.code {
                            0x93 => {
                                // Indirect Y
                                let pos: u8 = self.mem_read(self.program_counter);
                                let mem_address =
                                    self.mem_read_u16(pos as u16) + self.index_y as u16;
                                let data =
                                    self.accumulator & self.index_x & (mem_address >> 8) as u8;
                                self.mem_write(mem_address, data);
                            }
                            _ => {
                                // Absolute Y
                                let mem_address =
                                    self.mem_read_u16(self.program_counter) + self.index_y as u16;
                                let data =
                                    self.accumulator & self.index_x & (mem_address >> 8) as u8;
                                self.mem_write(mem_address, data);
                            }
                        }
                        opcode.cycles
                    }
                    "*SHX" => {
                        let mem_address =
                            self.mem_read_u16(self.program_counter) + self.index_y as u16;

                        let data = self.index_x & ((mem_address >> 8) as u8 + 1);
                        self.mem_write(mem_address, data);
                        opcode.cycles
                    }
                    _ => break Err(error::Error::UnhandledOpCode(instruction)),
                };
                if (cpu_cycles as i32 - consumed_cycles as i32) < 0 {
                    cpu_cycles = 0;
                    continue;
                } else {
                    cpu_cycles -= consumed_cycles as u32;
                }
                if program_counter_state == self.program_counter {
                    self.program_counter += (opcode.len - 1) as u16;
                }
            } else {
                break Err(error::Error::UnknownOpCode(instruction));
            }
        }
    }
    pub fn reset(&mut self) {
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
        let lo = self.stack_pointer as u16;
        let hi: u16 = 0x01 << 8;
        let addr = hi | lo;
        self.mem_read(addr)
    }
    /// Push data to the stack and increment the pointer.
    fn stack_push(&mut self, data: u8) {
        let lo = self.stack_pointer as u16;
        let hi: u16 = 0x01 << 8;
        let addr = hi | lo;
        self.mem_write(addr, data);
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
    }
    /// Push a 16-bit value to the stack
    fn stack_push_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.stack_push(hi);
        self.stack_push(lo);
    }
    /// Pop a 16-bit value from the stack
    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;
        hi << 8 | lo
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
    fn sub_from_accumulator(&mut self, value: u8) {
        let sum = self.accumulator.wrapping_add(value.wrapping_neg().wrapping_sub(if self.status.contains(CpuFlags::Carry) { 0 } else { 1 }));

        let carry = sum & 0b10000000 != 0;

        if carry {
            self.status.insert(CpuFlags::Carry);
        } else {
            self.status.remove(CpuFlags::Carry);
        }

        let result = sum as u8;

        if (value ^ result) & (result ^ self.accumulator) & 0x80 != 0 {
            self.status.insert(CpuFlags::Overflow);
        } else {
            self.status.remove(CpuFlags::Overflow);
        }

        self.accumulator = result;
        self.update_zero_negative_flags(self.accumulator);
    }
    /// Set the accumulator
    fn add_to_accumulator(&mut self, value: u8) {
        let sum = self.accumulator.wrapping_add(value).wrapping_add(if self.status.contains(CpuFlags::Carry) {
            1
        } else {
            0
        });

        let carry = sum & 0b10000000 != 0;

        if carry {
            self.status.insert(CpuFlags::Carry);
        } else {
            self.status.remove(CpuFlags::Carry);
        }

        let result = sum as u8;

        if (value ^ result) & (result ^ self.accumulator) & 0x80 != 0 {
            self.status.insert(CpuFlags::Overflow);
        } else {
            self.status.remove(CpuFlags::Overflow);
        }

        self.accumulator = result;
        self.update_zero_negative_flags(self.accumulator);
    }
    // MOS 6502 Instruction Handlers
    /// Add with Carry
    fn adc(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.add_to_accumulator(self.mem_read(addr));
        page_cross
    }
    /// Logical AND
    fn and(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.accumulator = self.accumulator & self.mem_read(addr);
        self.update_zero_negative_flags(self.accumulator);
        page_cross
    }
    /// Arithmetic Shift Left
    fn asl(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = if addr != 0 {
            self.mem_read(addr)
        } else {
            self.accumulator
        };
        if data >> 7 == 1 {
            self.sec();
        } else {
            self.clc();
        }
        let data = data << 1;
        if addr == 0 {
            self.accumulator = data;
        } else {
            self.mem_write(addr, data);
        }
        self.update_zero_negative_flags(data);
        data
    }
    /// Branch if Carry Clear
    fn bcc(&mut self) -> u8 {
        self.branch(!self.status.contains(CpuFlags::Carry))
    }
    /// Branch if Carry Set
    fn bcs(&mut self) -> u8 {
        self.branch(self.status.contains(CpuFlags::Carry))
    }
    /// Branch if Equal
    fn beq(&mut self) -> u8 {
        self.branch(self.status.contains(CpuFlags::Zero))
    }
    /// Bit Test
    fn bit(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        let mask = self.mem_read(addr);
        if self.accumulator & mask == 0 {
            self.status.insert(CpuFlags::Zero);
        } else {
            self.status.remove(CpuFlags::Zero);
        }
        if mask & 0b10000000 != 0 {
            self.status.insert(CpuFlags::Negative);
        } else {
            self.status.remove(CpuFlags::Negative);
        }
        if mask & 0b01000000 != 0 {
            self.status.insert(CpuFlags::Overflow);
        } else {
            self.status.remove(CpuFlags::Overflow);
        }
    }
    fn branch(&mut self, condition: bool) -> u8 {
        let mut result: u8 = 0;
        if condition {
            result += 1;
            let jump: i8 = self.mem_read(self.program_counter) as i8;
            let jump_addr = self
                .program_counter
                .wrapping_add(1)
                .wrapping_add(jump as u16);

            if self.program_counter.wrapping_add(1) & 0xff00 != jump_addr & 0xff00 {
                result += 1;
            }

            self.program_counter = jump_addr;
        }
        result
    }
    /// Branch if Minus
    fn bmi(&mut self) -> u8 {
        self.branch(self.status.contains(CpuFlags::Negative))
    }
    /// Branch if Not Equal
    fn bne(&mut self) -> u8 {
        self.branch(!self.status.contains(CpuFlags::Zero))
    }
    /// Branch if Positive
    fn bpl(&mut self) -> u8 {
        self.branch(!self.status.contains(CpuFlags::Negative))
    }
    /// Branch if Overflow Clear
    fn bvc(&mut self) -> u8 {
        self.branch(!self.status.contains(CpuFlags::Overflow))
    }
    /// Branch if Overflow Set
    fn bvs(&mut self) -> u8 {
        self.branch(self.status.contains(CpuFlags::Overflow))
    }
    /// Compare
    fn compare(&mut self, mode: &AddressingMode, target: u8) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        let negt = target.wrapping_sub(data) & 0b10000000 != 0;
        if target < data {
            self.status.remove(CpuFlags::Carry);
            self.status.remove(CpuFlags::Zero);
        } else if target > data {
            self.status.insert(CpuFlags::Carry);
            self.status.remove(CpuFlags::Zero);
        } else {
            self.status.insert(CpuFlags::Carry);
            self.status.insert(CpuFlags::Zero);
        }
        // Set Negative Flag
        if target > data || target < data {
            if negt {
                self.status.insert(CpuFlags::Negative);
            } else {
                self.status.remove(CpuFlags::Negative);
            }
        } else {
            self.status.remove(CpuFlags::Negative);
        }
        page_cross
    }
    /// Decrement Memory
    fn dec(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        let data = self.mem_read(addr).wrapping_sub(1);
        self.mem_write(addr, data);
        self.update_zero_negative_flags(data);
    }
    /// Decrement Index Register X
    fn dex(&mut self) {
        self.index_x = self.index_x.wrapping_sub(1);
        self.update_zero_negative_flags(self.index_x);
    }
    /// Decrement Index Register Y
    fn dey(&mut self) {
        self.index_y = self.index_y.wrapping_sub(1);
        self.update_zero_negative_flags(self.index_y);
    }
    /// Exclusive OR
    fn eor(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.accumulator = self.accumulator & data;
        self.update_zero_negative_flags(self.accumulator);
        page_cross
    }
    /// Increment Memory
    fn inc(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = self.mem_read(addr).wrapping_add(1);
        self.mem_write(addr, data);
        self.update_zero_negative_flags(data);
        data
    }
    /// Increment Index Register X
    fn inx(&mut self) {
        self.index_x = self.index_x.wrapping_add(1);
        self.update_zero_negative_flags(self.index_x);
    }
    /// Increment Index Register Y
    fn iny(&mut self) {
        self.index_y = self.index_y.wrapping_add(1);
        self.update_zero_negative_flags(self.index_y);
    }
    /// Jump
    fn jmp(&mut self) {
        self.program_counter = self.mem_read_u16(self.program_counter);
    }
    /// Jump to Subroutine
    fn jsr(&mut self) {
        self.stack_push_u16(self.program_counter + 2 - 1);
        self.program_counter = self.mem_read_u16(self.program_counter);
    }
    /// Load Accumulator
    fn lda(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.accumulator = self.mem_read(addr);
        self.update_zero_negative_flags(self.accumulator);
        page_cross
    }
    /// Load X Register
    fn ldx(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.index_x = self.mem_read(addr);
        self.update_zero_negative_flags(self.index_x);
        page_cross
    }
    /// Load Y Register
    fn ldy(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.index_y = self.mem_read(addr);
        self.update_zero_negative_flags(self.index_y);
        page_cross
    }
    /// Logical Shift Right
    fn lsr(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = if addr != 0 {
            self.mem_read(addr)
        } else {
            self.accumulator
        };
        if data & 0b00000001 == 1 {
            self.sec();
        } else {
            self.clc();
        }
        let data = data >> 1;
        if addr == 0 {
            self.accumulator = data;
        } else {
            self.mem_write(addr, data);
        }
        self.update_zero_negative_flags(data);
        data
    }
    /// Logical Inclusive OR
    fn ora(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        self.accumulator = self.accumulator & self.mem_read(addr);
        self.update_zero_negative_flags(self.accumulator);
        page_cross
    }
    /// Push Accumulator
    fn pha(&mut self) {
        self.stack_push(self.accumulator);
    }
    /// Push Processor Status
    fn php(&mut self) {
        self.stack_push(self.status.bits());
    }
    /// Pull accumulator
    fn pla(&mut self) {
        self.accumulator = self.stack_pop();
        self.update_zero_negative_flags(self.accumulator);
    }
    /// Pull processor status
    fn plp(&mut self) {
        self.status = BitFlags::from_bits_truncate(self.stack_pop());
    }
    /// Rotate Left
    fn rol(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = if addr != 0 {
            self.mem_read(addr)
        } else {
            self.accumulator
        };
        let carry = self.status.contains(CpuFlags::Carry);
        if data & 0b10000000 == 1 {
            self.sec();
        } else {
            self.clc();
        }
        let mut data = data << 1;
        if carry {
            data |= 0b00000001;
        }
        if addr == 0 {
            self.accumulator = data;
        } else {
            self.mem_write(addr, data);
        }
        self.update_zero_negative_flags(data);
        data
    }
    /// Rotate Right
    fn ror(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = if addr != 0 {
            self.mem_read(addr)
        } else {
            self.accumulator
        };
        let carry = self.status.contains(CpuFlags::Carry);
        if data & 0b00000001 == 1 {
            self.sec();
        } else {
            self.clc();
        }
        let mut data = data >> 1;
        if carry {
            data |= 0b10000000;
        }
        if addr == 0 {
            self.accumulator = data;
        } else {
            self.mem_write(addr, data);
        }
        self.update_zero_negative_flags(data);
        data
    }
    /// Return from interrupt
    fn rti(&mut self) {
        self.plp();
        self.status.remove(CpuFlags::Break);
        self.status.insert(CpuFlags::UNK);
        self.program_counter = self.stack_pop_u16();
    }
    /// Return from subroutine
    fn rts(&mut self) {
        self.program_counter = self.stack_pop_u16() + 1;
    }
    /// Subtract with carry
    fn sbc(&mut self, mode: &AddressingMode) -> bool {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.sub_from_accumulator(data);
        page_cross
    }
    /// Store accumulator
    fn sta(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.accumulator);
    }
    /// Store Register X
    fn stx(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.index_x);
    }
    /// Store Register Y
    fn sty(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.index_y);
    }
    /// Transfer Accumulator to X
    fn tax(&mut self) {
        self.index_x = self.accumulator;
        self.update_zero_negative_flags(self.index_x);
    }
    /// Transfer Accumulator to Y
    fn tay(&mut self) {
        self.index_y = self.accumulator;
        self.update_zero_negative_flags(self.index_y);
    }
    /// Transfer stack pointer to X
    fn tsx(&mut self) {
        self.index_x = self.stack_pointer;
        self.update_zero_negative_flags(self.index_x);
    }
    /// Transfer X to accumulator
    fn txa(&mut self) {
        self.accumulator = self.index_x;
        self.update_zero_negative_flags(self.accumulator);
    }
    /// Transfer X to stack pointer
    fn txs(&mut self) {
        self.stack_pointer = self.index_x;
    }
    /// Transfer Y to accumulator
    fn tya(&mut self) {
        self.accumulator = self.index_y;
        self.update_zero_negative_flags(self.accumulator);
    }
}

#[cfg(test)]
mod tests {
    struct RAMChip {
        prg_rom: Vec<u8>,
        _chr_rom: Vec<u8>,
        memory: [u8; 0xFFFF],
    }
    impl RAMChip {
        pub fn new(prg_rom: Vec<u8>, _chr_rom: Vec<u8>) -> Arc<RwLock<Self>> {
            Arc::new(RwLock::new(RAMChip {
                memory: [0; 0xFFFF],
                prg_rom,
                _chr_rom,
            }))
        }
    }
    impl Memory for RAMChip {
        fn mem_read(&self, addr: u16) -> u8 {
            let mut addr = addr;
            match addr {
                0x8000..=0xFFFF => {
                    addr -= 0x8000;
                    if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
                        addr = addr % 0x4000;
                    }
                    self.prg_rom[addr as usize]
                }
                _ => self.memory[addr as usize],
            }
        }
        fn mem_write(&mut self, addr: u16, data: u8) {
            match addr {
                0x8000..=0xFFFF => panic!("write to cartridge rom"),
                _ => self.memory[addr as usize] = data,
            }
        }
    }
    use super::*;
    use crate::bus::{Bus, MemoryMapper};
    use crate::memory::Memory;
    #[test]
    fn test_signed() {
        let mut bus = Bus::new();
        let ram = RAMChip::new(vec![], vec![]);

        bus.map_region(0, 0xFFFF, ram.clone()).unwrap(); // Add ram chip
        bus.mem_write_signed(0x01ff, -45);
        bus.mem_write_signed(0x01fe, 56);
        bus.mem_write(0x01fd, 224);

        let signed_a = bus.mem_read_signed(0x01ff);
        let signed_b = bus.mem_read_signed(0x01fe);
        let unsigned_c = bus.mem_read(0x01fd);
        let signed_c = bus.mem_read_signed(0x01fd);
        assert_eq!(signed_a, -45);
        assert_eq!(signed_b, 56);
        assert_eq!(unsigned_c, 224);
        assert_eq!(signed_c, -32);
    }
}

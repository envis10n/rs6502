use rs6502::{
    bus::{Bus, MemoryMapper},
    memory::Memory,
    CpuFlags, Mos6502,
};
use std::sync::{Arc, RwLock};

const NES_TAG: [u8; 4] = [0x4e, 0x45, 0x53, 0x1a];
const PRG_ROM_PAGE_SIZE: usize = 16384;
const CHR_ROM_PAGE_SIZE: usize = 8192;

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;

enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
}

struct Rom {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
    pub screen_mirroring: Mirroring,
}

impl Rom {
    pub fn new(raw: &[u8]) -> Result<Arc<RwLock<Rom>>, String> {
        if raw[0..4] != NES_TAG {
            return Err("File is not in iNES format".to_string());
        }
        let mapper = (raw[7] & 0b11110000) | (raw[6] >> 4);

        let ines_ver = (raw[7] >> 2) & 0b11;
        if ines_ver != 0 {
            return Err("NES2.0 format is unsupported".to_string());
        }

        let four_screen = raw[6] & 0b1000 != 0;
        let vertical_mirroring = raw[6] & 0b1 != 0;
        let screen_mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        let prg_rom_size = raw[4] as usize * PRG_ROM_PAGE_SIZE;
        let chr_rom_size = raw[5] as usize * CHR_ROM_PAGE_SIZE;

        let skip_trainer = raw[6] & 0b100 != 0;

        let prg_rom_start: usize = 16 + if skip_trainer { 512 } else { 0 };
        let chr_rom_start: usize = prg_rom_start + prg_rom_size;

        Ok(Arc::new(RwLock::new(Rom {
            prg_rom: raw[prg_rom_start..(prg_rom_start + prg_rom_size)].to_vec(),
            chr_rom: raw[chr_rom_start..(chr_rom_start + chr_rom_size)].to_vec(),
            mapper,
            screen_mirroring,
        })))
    }
}

impl Memory for Rom {
    fn mem_read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                let mut rom_addr = addr - 0x8000;
                if self.prg_rom.len() == 0x4000 && rom_addr >= 0x4000 {
                    rom_addr = rom_addr % 0x4000;
                }
                self.prg_rom[rom_addr as usize]
            }
            _ => 0,
        }
    }
    fn mem_write(&mut self, addr: u16, data: u8) {
        panic!("Unable to write to rom");
    }
}

struct RAMChip {
    memory: [u8; 2048],
}
impl RAMChip {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(RAMChip { memory: [0; 2048] }))
    }
}
impl Memory for RAMChip {
    fn mem_read(&self, addr: u16) -> u8 {
        let mirror_down_addr = addr & 0b00000111_11111111;
        self.memory[mirror_down_addr as usize]
    }
    fn mem_write(&mut self, addr: u16, data: u8) {
        let mirror_down_addr = addr & 0b11111111111;
        self.memory[mirror_down_addr as usize] = data;
    }
}

pub fn main() {
    let bytes = std::fs::read("nestest.nes").unwrap();

    let rom = Rom::new(&bytes).unwrap();
    let mut bus = Bus::new();
    let ram = RAMChip::new();

    bus.map_region(RAM, RAM_MIRRORS_END, ram.clone()).unwrap();
    bus.map_region(0x8000, 0xFFFF, rom.clone()).unwrap();

    let cpu = Mos6502::new(bus.clone());
    {
        let mut mos = cpu.lock().unwrap();
        (*mos).reset();
        (*mos).program_counter = 0xc000;
        loop {
            if (*mos).status.contains(CpuFlags::Break) {
                break;
            }
            if let Err(err) = (*mos).execute() {
                panic!("{}", err);
            }
        }
    }
}

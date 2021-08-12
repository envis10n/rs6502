pub trait Memory {
    fn mem_read(&self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, data: u8);
    fn mem_read_signed(&self, addr: u16) -> i8 {
        let data = self.mem_read(addr);
        -(256 - data as i16) as i8
    }
    fn mem_write_signed(&mut self, addr: u16, data: i8) {
        if data < 0 {
            // negative
            let data = (256 + data as i16) as u8;
            self.mem_write(addr, data);
        } else {
            self.mem_write(addr, data as u8)
        }
    }
    fn mem_read_u16(&self, addr: u16) -> u16 {
        let lo = self.mem_read(addr);
        let hi = self.mem_read(addr + 1);
        u16::from_le_bytes([lo, hi])
    }
    fn mem_read_u16_wrap(&self, addr: u16) -> u16 {
        let addr_hi = (addr >> 8) as u8;
        let mut addr_lo = (addr & 0xFF) as u8;
        addr_lo = addr_lo.wrapping_add(1);
        let next_addr = u16::from_le_bytes([addr_lo, addr_hi]);
        let lo = self.mem_read(addr);
        let hi = self.mem_read(next_addr);
        u16::from_le_bytes([lo, hi])
    }
    fn mem_write_u16(&mut self, addr: u16, data: u16) {
        let bytes: [u8;2] = data.to_le_bytes();
        self.mem_write(addr, bytes[0]);
        self.mem_write(addr + 1, bytes[1]);
    }
}
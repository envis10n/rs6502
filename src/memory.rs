pub trait Memory {
    fn mem_read(&self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, data: u8);
    fn mem_read_u16(&self, addr: u16) -> u16 {
        let lo = self.mem_read(addr);
        let hi = self.mem_read(addr + 1);
        u16::from_le_bytes([lo, hi])
    }
    fn mem_write_u16(&mut self, addr: u16, data: u16) {
        let bytes: [u8;2] = data.to_le_bytes();
        self.mem_write(addr, bytes[0]);
        self.mem_write(addr + 1, bytes[1]);
    }
}
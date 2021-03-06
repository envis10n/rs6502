use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    None,
    UnknownOpCode(u8),
    UnhandledOpCode(u8),
    MemoryMapConflict((u16, u16))
}

#[allow(unreachable_patterns)]
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::None => write!(f, "an error occurred"),
            Error::UnhandledOpCode(code) => write!(f, "unhandled opcode: 0x{:2X}", code),
            Error::UnknownOpCode(code) => write!(f, "unknown opcode: 0x{:2X}", code),
            Error::MemoryMapConflict(key) => write!(f, "memory region is already mapped: ${:04X}-${:04X}", key.0, key.1),
            _ => todo!()
        }
    }
}

impl StdError for Error {}
#[cfg(feature = "trace")]
pub trait Trace {
    fn trace(&self) -> String;
}

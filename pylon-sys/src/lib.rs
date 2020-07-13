#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

impl Default for PylonGrabResult_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[cfg(test)]
mod tests {
    use super::{PylonInitialize, PylonTerminate};

    #[test]
    fn init() {
        unsafe {
            assert_eq!(PylonInitialize(), 0);
            assert_eq!(PylonTerminate(), 0);
        }
    }
}

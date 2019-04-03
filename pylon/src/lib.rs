use pylon_sys;
use std::ffi::CString;
use std::sync::{Once, ONCE_INIT};
use std::{error::Error, fmt};

static PYLON_INITIALIZED: Once = ONCE_INIT;

pub fn initialize() {
    PYLON_INITIALIZED.call_once(|| unsafe {
        if pylon_sys::PylonInitialize() != 0 {
            panic!("Can't initialize Pylon");
        }
    })
}

#[derive(Debug)]
pub struct PylonError {
    api_msg: String,
}

impl PylonError {
    fn with_last_error() -> PylonError {
        let mut len = 0;
        unsafe {
            pylon_sys::GenApiGetLastErrorMessage(&mut 0, &mut len);
        }
        let mut buff = Vec::with_capacity(len);

        let api_msg = unsafe {
            pylon_sys::GenApiGetLastErrorMessage(buff.as_mut_ptr() as *mut i8, &mut len);
            CString::from_vec_unchecked(buff).into_string().unwrap()
        };

        PylonError { api_msg }
    }
}

impl Error for PylonError {}

impl fmt::Display for PylonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.api_msg)
    }
}

pub struct Device {
    handle: pylon_sys::PYLON_DEVICE_HANDLE,
}

macro_rules! check_res {
    ($res:expr, $ret:expr) => {
        if $res != 0 {
            Err(PylonError::with_last_error())
        } else {
            Ok($ret)
        }
    };
}

impl Device {
    pub fn enumerate_devices() -> Result<usize, PylonError> {
        let mut devices = 0;
        let res = unsafe { pylon_sys::PylonEnumerateDevices(&mut devices) };
        check_res!(res, devices)
    }

    pub fn crate_device_by_index(idx: usize) -> Result<Self, PylonError> {
        let mut handle = 0;
        let res = unsafe { pylon_sys::PylonCreateDeviceByIndex(idx, &mut handle) };
        check_res!(res, Device { handle })
    }

    pub fn open(&mut self) -> Result<(), PylonError> {
        let res = unsafe {
            pylon_sys::PylonDeviceOpen(
                self.handle,
                (pylon_sys::PYLONC_ACCESS_MODE_CONTROL | pylon_sys::PYLONC_ACCESS_MODE_STREAM)
                    as i32,
            )
        };
        check_res!(res, ())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

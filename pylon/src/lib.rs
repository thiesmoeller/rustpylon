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
    errno: pylon_sys::HRESULT,
    api_msg: String,
    api_detail: String,
}

impl PylonError {
    fn with_last_error(res: pylon_sys::HRESULT) -> PylonError {
        let mut len = 0;
        unsafe {
            pylon_sys::GenApiGetLastErrorMessage(&mut 0, &mut len);
        }
        let mut buff = Vec::with_capacity(len);
        let api_msg = unsafe {
            pylon_sys::GenApiGetLastErrorMessage(buff.as_mut_ptr() as *mut i8, &mut len);
            CString::from_vec_unchecked(buff).into_string().unwrap()
        };

        unsafe {
            pylon_sys::GenApiGetLastErrorDetail(&mut 0, &mut len);
        }

        let mut buff = Vec::with_capacity(len);
        let api_detail = unsafe {
            pylon_sys::GenApiGetLastErrorDetail(buff.as_mut_ptr() as *mut i8, &mut len);
            CString::from_vec_unchecked(buff).into_string().unwrap()
        };

        PylonError {
            errno: res,
            api_msg,
            api_detail,
        }
    }
}

impl Error for PylonError {}

impl fmt::Display for PylonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Error ({}): {} ({})",
            &self.errno, &self.api_msg, &self.api_detail
        )
    }
}

#[derive(Debug)]
pub struct Device {
    handle: pylon_sys::PYLON_DEVICE_HANDLE,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DD")
    }
}

macro_rules! check_res {
    ($res:expr, $ret:expr) => {
        if $res != 0 {
            Err(PylonError::with_last_error($res))
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

    pub fn create_device_by_index(idx: usize) -> Result<Self, PylonError> {
        let mut handle = 0;
        let res = unsafe { pylon_sys::PylonCreateDeviceByIndex(idx, &mut handle) };
        check_res!(res, Device { handle })
    }

    pub fn open(&self) -> Result<(), PylonError> {
        let res = unsafe {
            pylon_sys::PylonDeviceOpen(
                self.handle,
                (pylon_sys::PYLONC_ACCESS_MODE_CONTROL | pylon_sys::PYLONC_ACCESS_MODE_STREAM)
                    as i32,
            )
        };
        check_res!(res, ())
    }

    pub fn feature_is_available(&self, feat: &str) -> bool {
        unsafe { pylon_sys::PylonDeviceFeatureIsAvailable(self.handle, feat.as_ptr() as *mut i8) }
    }

    pub fn set_string_feature(&self, key: &str, value: &str) -> Result<(), PylonError> {
        let res = unsafe {
            pylon_sys::PylonDeviceFeatureFromString(
                self.handle,
                key.as_ptr() as *mut i8,
                value.as_ptr() as *mut i8,
            )
        };
        check_res!(res, ())
    }

    pub fn set_integer_feature(&self, key: &str, value: i64) -> Result<(), PylonError> {
        let res = unsafe {
            if pylon_sys::PylonDeviceFeatureIsWritable(self.handle, key.as_ptr() as *mut i8) {
                1000
            } else {
                pylon_sys::PylonDeviceSetIntegerFeature(self.handle, key.as_ptr() as *mut i8, value)
            }
        };
        check_res!(res, ())
    }

    pub fn get_integer_feature(&self, key: &str) -> Result<i64, PylonError> {
        let mut value = 0;
        let res = unsafe {
            if pylon_sys::PylonDeviceFeatureIsReadable(self.handle, key.as_ptr() as *mut i8) {
                1000
            } else {
                pylon_sys::PylonDeviceGetIntegerFeature(self.handle, key.as_ptr() as *mut i8, &mut value)
            }
        };
        check_res!(res, value)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

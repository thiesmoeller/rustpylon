use image::{DynamicImage, GrayImage};
use pylon_sys::{self, EPylonGrabStatus, EPylonPixelType};
use std::ffi::{c_void, CString};
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

    fn with_msg(msg: &str) -> PylonError {
        PylonError {
            errno: 1000,
            api_msg: msg.to_owned(),
            api_detail: "".to_owned(),
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
    grab_buffer: Vec<u8>,
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
        check_res!(
            res,
            Device {
                handle,
                grab_buffer: Vec::new()
            }
        )
    }

    pub fn open(&mut self) -> Result<(), PylonError> {
        let res = unsafe {
            pylon_sys::PylonDeviceOpen(
                self.handle,
                (pylon_sys::PYLONC_ACCESS_MODE_CONTROL | pylon_sys::PYLONC_ACCESS_MODE_STREAM)
                    as i32,
            )
        };
        check_res!(res, ())?;

        // query payload size and allocate memory for grabbing frames
        let payload_size = self.get_integer_feature("PayloadSize")?;
        self.grab_buffer.resize(payload_size as usize, 0);
        Ok(())
    }

    pub fn feature_is_available(&self, feat: &str) -> bool {
        let feat = CString::new(feat).unwrap();
        unsafe { pylon_sys::PylonDeviceFeatureIsAvailable(self.handle, feat.as_ptr()) }
    }

    pub fn set_string_feature(&self, key: &str, value: &str) -> Result<(), PylonError> {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();
        let res = unsafe {
            pylon_sys::PylonDeviceFeatureFromString(
                self.handle,
                key.as_ptr(),
                value.as_ptr() as *mut i8,
            )
        };
        check_res!(res, ())
    }

    pub fn set_integer_feature(&self, key: &str, value: i64) -> Result<(), PylonError> {
        let key = CString::new(key).unwrap();
        unsafe {
            if !pylon_sys::PylonDeviceFeatureIsWritable(self.handle, key.as_ptr()) {
                Err(PylonError::with_msg(&format!(
                    "Device Feature: {:?} is not writable.",
                    key
                )))
            } else {
                let res = pylon_sys::PylonDeviceSetIntegerFeature(self.handle, key.as_ptr(), value);
                check_res!(res, ())
            }
        }
    }

    pub fn get_string_feature(&self, key: &str) -> Result<String, PylonError> {
        let key = CString::new(key).unwrap();
        unsafe {
            if !pylon_sys::PylonDeviceFeatureIsReadable(self.handle, key.as_ptr()) {
                Err(PylonError::with_msg(&format!(
                    "Device Feature: {:?} is not readable.",
                    key
                )))
            } else {
                let value = vec![0u8; 256];
                let mut size = value.len();
                let res = pylon_sys::PylonDeviceFeatureToString(
                    self.handle,
                    key.as_ptr(),
                    value.as_ptr() as *mut i8,
                    &mut size,
                );
                check_res!(res, String::from_utf8(value).unwrap())
            }
        }
    }

    pub fn get_integer_feature(&self, key: &str) -> Result<i64, PylonError> {
        let key = CString::new(key).unwrap();
        unsafe {
            if !pylon_sys::PylonDeviceFeatureIsReadable(self.handle, key.as_ptr()) {
                Err(PylonError::with_msg(&format!(
                    "Device Feature: {:?} is not readable.",
                    key
                )))
            } else {
                let mut value = 0;
                let res =
                    pylon_sys::PylonDeviceGetIntegerFeature(self.handle, key.as_ptr(), &mut value);
                check_res!(res, value)
            }
        }
    }

    pub fn grab_single_frame(&mut self) -> Result<DynamicImage, PylonError> {
        let mut buffer_ready = false;
        let mut grab_result = pylon_sys::PylonGrabResult_t::default();
        let res = unsafe {
            pylon_sys::PylonDeviceGrabSingleFrame(
                self.handle,
                0,
                self.grab_buffer.as_mut_ptr() as *mut c_void,
                self.grab_buffer.len(),
                &mut grab_result,
                &mut buffer_ready,
                500,
            )
        };

        check_res!(res, ())?;

        if !buffer_ready {
            Err(PylonError::with_msg("Grabbing timeout"))
        } else if grab_result.Status != EPylonGrabStatus::Grabbed {
            Err(PylonError::with_msg(&format!(
                "Frame was not grabbed successfully. Status: {:#?}",
                grab_result.Status
            )))
        } else {
            match grab_result.PixelType {
                EPylonPixelType::PixelType_Mono8 => GrayImage::from_vec(
                    grab_result.SizeX as u32,
                    grab_result.SizeY as u32,
                    self.grab_buffer.clone(),
                )
                .map_or(Err(PylonError::with_last_error(1004)), |i| {
                    Ok(DynamicImage::ImageLuma8(i))
                }),
                _ => unimplemented!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

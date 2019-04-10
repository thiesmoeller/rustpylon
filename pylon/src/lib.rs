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

macro_rules! check_res {
    ($res:expr, $ret:expr) => {
        if $res != 0 {
            dbg!($res);
            Err(PylonError::with_last_error($res))
        } else {
            Ok($ret)
        }
    };
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

pub struct StreamGrabber {
    handle: pylon_sys::PYLON_STREAMGRABBER_HANDLE,
    waitobject: pylon_sys::PYLON_WAITOBJECT_HANDLE,
    grab_buffers: Vec<(pylon_sys::PYLON_STREAMBUFFER_HANDLE, Vec<u8>)>,
}

impl StreamGrabber {
    pub fn grab(&mut self) -> Result<DynamicImage, PylonError> {
        let mut is_ready = false;
        let res = unsafe { pylon_sys::PylonWaitObjectWait(self.waitobject, 1000, &mut is_ready) };

        check_res!(res, ())?;

        if !is_ready {
            return Err(PylonError::with_msg("Timeout grabbing images"));
        }

        let mut grab_result = pylon_sys::PylonGrabResult_t::default();
        let res = unsafe {
            pylon_sys::PylonStreamGrabberRetrieveResult(
                self.handle,
                &mut grab_result,
                &mut is_ready,
            )
        };

        check_res!(res, ())?;

        let buffer_idx = grab_result.Context as usize;
        let image_res = match grab_result.PixelType {
            EPylonPixelType::PixelType_Mono8 => GrayImage::from_vec(
                grab_result.SizeX as u32,
                grab_result.SizeY as u32,
                self.grab_buffers[buffer_idx].1.clone(),
            )
            .map_or(Err(PylonError::with_last_error(1004)), |i| {
                Ok(DynamicImage::ImageLuma8(i))
            }),
            _ => unimplemented!(),
        };

        let res = unsafe {
            pylon_sys::PylonStreamGrabberQueueBuffer(
                self.handle,
                self.grab_buffers[buffer_idx].0,
                buffer_idx as *const c_void,
            )
        };

        check_res!(res, ())?;

        image_res
    }
}

#[derive(Debug)]
pub struct Device {
    handle: pylon_sys::PYLON_DEVICE_HANDLE,
    grab_buffer: Vec<u8>,
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

    pub fn get_stream_grabber(&mut self, channel: usize) -> Result<StreamGrabber, PylonError> {
        if self.stream_grabber_channels()? <= channel {
            return Err(PylonError::with_msg(&format!(
                "No streamgrabber channel {} available.",
                channel
            )));
        }

        let mut grabber_handle = 0;
        let res = unsafe {
            pylon_sys::PylonDeviceGetStreamGrabber(self.handle, channel, &mut grabber_handle)
        };

        check_res!(res, ())?;

        let res = unsafe { pylon_sys::PylonStreamGrabberOpen(grabber_handle) };

        check_res!(res, ())?;

        let mut waitobject = 0;
        let res =
            unsafe { pylon_sys::PylonStreamGrabberGetWaitObject(grabber_handle, &mut waitobject) };

        check_res!(res, ())?;

        let payload_size = self.get_integer_feature("PayloadSize")?;

        const BUFFERS: usize = 5;
        let mut grab_buffers = Vec::new();
        (0..BUFFERS).for_each(|_| grab_buffers.push((0, vec![0; payload_size as usize])));

        let res = unsafe { pylon_sys::PylonStreamGrabberSetMaxNumBuffer(grabber_handle, BUFFERS) };

        check_res!(res, ())?;

        let res = unsafe {
            pylon_sys::PylonStreamGrabberSetMaxBufferSize(grabber_handle, payload_size as usize)
        };

        check_res!(res, ())?;

        let res = unsafe { pylon_sys::PylonStreamGrabberPrepareGrab(grabber_handle) };

        check_res!(res, ())?;

        for i in 0..BUFFERS {
            unsafe {
                let res = pylon_sys::PylonStreamGrabberRegisterBuffer(
                    grabber_handle,
                    grab_buffers[i].1.as_mut_ptr() as *mut c_void,
                    payload_size as usize,
                    &mut grab_buffers[i].0,
                );
                check_res!(res, ())?;

                let res = pylon_sys::PylonStreamGrabberQueueBuffer(
                    grabber_handle,
                    grab_buffers[i].0,
                    i as *const c_void,
                );
                check_res!(res, ())?;
            }
        }

        check_res!(
            res,
            StreamGrabber {
                handle: grabber_handle,
                waitobject,
                grab_buffers
            }
        )
    }

    pub fn execute_command(&mut self, cmd: &str) -> Result<(), PylonError> {
        let cmd = CString::new(cmd).unwrap();
        let res = unsafe { pylon_sys::PylonDeviceExecuteCommandFeature(self.handle, cmd.as_ptr()) };

        check_res!(res, ())
    }

    pub fn stream_grabber_channels(&mut self) -> Result<usize, PylonError> {
        let mut channels = 0;
        let res = unsafe {
            pylon_sys::PylonDeviceGetNumStreamGrabberChannels(self.handle, &mut channels)
        };

        check_res!(res, channels)
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

    pub fn set_float_feature(&self, key: &str, value: f64) -> Result<(), PylonError> {
        let key = CString::new(key).unwrap();
        unsafe {
            if !pylon_sys::PylonDeviceFeatureIsWritable(self.handle, key.as_ptr()) {
                Err(PylonError::with_msg(&format!(
                    "Device Feature: {:?} is not writable.",
                    key
                )))
            } else {
                let res = pylon_sys::PylonDeviceSetFloatFeature(self.handle, key.as_ptr(), value);
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

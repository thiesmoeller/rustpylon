use core::pin::Pin;
use core::task::{Context, Poll};
use image::{DynamicImage, GrayImage};
use pylon_sys::{self, EPylonGrabStatus, EPylonPixelType};
use std::ffi::{c_void, CString};
use std::sync::Once;
use std::{error::Error, fmt};

static PYLON_INITIALIZED: Once = Once::new();

pub fn initialize() {
    PYLON_INITIALIZED.call_once(|| unsafe {
        if pylon_sys::PylonInitialize() != 0 {
            panic!("Can't initialize Pylon");
        }
    })
}

macro_rules! checked {
    ($res:expr, $ret:expr) => {
        unsafe {
            if $res != 0 {
                dbg!($res);
                Err(PylonError::with_last_error($res))
            } else {
                Ok($ret)
            }
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
        let mut buff = Vec::with_capacity(len as usize);
        let api_msg = unsafe {
            pylon_sys::GenApiGetLastErrorMessage(buff.as_mut_ptr() as *mut i8, &mut len);
            CString::from_vec_unchecked(buff).into_string().unwrap()
        };

        unsafe {
            pylon_sys::GenApiGetLastErrorDetail(&mut 0, &mut len);
        }

        let mut buff = Vec::with_capacity(len as usize);
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

impl tokio::stream::Stream for StreamGrabber {
    type Item = Result<DynamicImage, PylonError>;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut is_ready = false;
        checked!(
            pylon_sys::PylonWaitObjectWait(self.waitobject, 1, &mut is_ready),
            ()
        )?;
        if is_ready {
            return Poll::Ready(Some(self.grab()));
        }

        ctx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl StreamGrabber {
    pub fn grab(&mut self) -> Result<DynamicImage, PylonError> {
        let mut is_ready = false;
        checked!(
            pylon_sys::PylonWaitObjectWait(self.waitobject, 1000, &mut is_ready),
            ()
        )?;

        if !is_ready {
            return Err(PylonError::with_msg("Timeout grabbing images"));
        }

        let mut grab_result = pylon_sys::PylonGrabResult_t::default();
        checked!(
            pylon_sys::PylonStreamGrabberRetrieveResult(
                self.handle,
                &mut grab_result,
                &mut is_ready,
            ),
            ()
        )?;

        if grab_result.Status != pylon_sys::EPylonGrabStatus::Grabbed {
            return Err(PylonError::with_msg("Grabbing failed for frame."));
        }

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

        checked!(
            pylon_sys::PylonStreamGrabberQueueBuffer(
                self.handle,
                self.grab_buffers[buffer_idx].0,
                buffer_idx as *const c_void,
            ),
            ()
        )?;

        image_res
    }
}

#[derive(Debug)]
pub struct Device {
    handle: pylon_sys::PYLON_DEVICE_HANDLE,
    pub serial_number: String,
    grab_buffer: Vec<u8>,
}

impl Device {
    pub fn enumerate_devices() -> Result<usize, PylonError> {
        let mut devices = 0;
        checked!(
            pylon_sys::PylonEnumerateDevices(&mut devices),
            devices as usize
        )
    }

    pub fn create_device_by_index(idx: usize) -> Result<Self, PylonError> {
        if idx >= Self::enumerate_devices()? {
            return Err(PylonError::with_msg(&format!(
                "No device with index: {} available.",
                idx
            )));
        }

        let mut handle = 0;
        checked!(pylon_sys::PylonCreateDeviceByIndex(idx, &mut handle), ());

        let mut info_handle = pylon_sys::PylonDeviceInfo_t::default();
        checked!(
            pylon_sys::PylonDeviceGetDeviceInfo(handle, &mut info_handle),
            ()
        );

        let dev_serial = unsafe { std::ffi::CStr::from_ptr(info_handle.SerialNumber.as_ptr()) };
        Ok(Device {
            handle,
            serial_number: dev_serial.to_str().unwrap().to_string(),
            grab_buffer: Vec::new(),
        })
    }

    pub fn open(&mut self) -> Result<(), PylonError> {
        checked!(
            pylon_sys::PylonDeviceOpen(
                self.handle,
                (pylon_sys::PYLONC_ACCESS_MODE_CONTROL | pylon_sys::PYLONC_ACCESS_MODE_STREAM)
                    as i32,
            ),
            ()
        )?;

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
        checked!(
            pylon_sys::PylonDeviceGetStreamGrabber(self.handle, channel, &mut grabber_handle),
            ()
        )?;

        checked!(pylon_sys::PylonStreamGrabberOpen(grabber_handle), ())?;

        let mut waitobject = 0;
        checked!(
            pylon_sys::PylonStreamGrabberGetWaitObject(grabber_handle, &mut waitobject),
            ()
        )?;

        let payload_size = self.get_integer_feature("PayloadSize")?;

        const BUFFERS: usize = 5;
        let mut grab_buffers = Vec::new();
        (0..BUFFERS).for_each(|_| grab_buffers.push((0, vec![0; payload_size as usize])));

        checked!(
            pylon_sys::PylonStreamGrabberSetMaxNumBuffer(grabber_handle, BUFFERS),
            ()
        )?;

        checked!(
            pylon_sys::PylonStreamGrabberSetMaxBufferSize(grabber_handle, payload_size as usize),
            ()
        )?;

        checked!(pylon_sys::PylonStreamGrabberPrepareGrab(grabber_handle), ())?;

        for i in 0..BUFFERS {
            checked!(
                pylon_sys::PylonStreamGrabberRegisterBuffer(
                    grabber_handle,
                    grab_buffers[i].1.as_mut_ptr() as *mut c_void,
                    payload_size as usize,
                    &mut grab_buffers[i].0,
                ),
                ()
            )?;

            checked!(
                pylon_sys::PylonStreamGrabberQueueBuffer(
                    grabber_handle,
                    grab_buffers[i].0,
                    i as *const c_void,
                ),
                ()
            )?;
        }

        Ok(StreamGrabber {
            handle: grabber_handle,
            waitobject,
            grab_buffers,
        })
    }

    pub fn execute_command(&mut self, cmd: &str) -> Result<(), PylonError> {
        let cmd = CString::new(cmd).unwrap();
        checked!(
            pylon_sys::PylonDeviceExecuteCommandFeature(self.handle, cmd.as_ptr()),
            ()
        )
    }

    pub fn stream_grabber_channels(&mut self) -> Result<usize, PylonError> {
        let mut channels = 0;
        checked!(
            pylon_sys::PylonDeviceGetNumStreamGrabberChannels(self.handle, &mut channels),
            channels as usize
        )
    }

    pub fn feature_is_available(&self, feat: &str) -> bool {
        let feat = CString::new(feat).unwrap();
        unsafe { pylon_sys::PylonDeviceFeatureIsAvailable(self.handle, feat.as_ptr()) }
    }

    pub fn set_string_feature(&self, key: &str, value: &str) -> Result<(), PylonError> {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();
        checked!(
            pylon_sys::PylonDeviceFeatureFromString(
                self.handle,
                key.as_ptr(),
                value.as_ptr() as *mut i8
            ),
            ()
        )
    }

    pub fn set_integer_feature(&self, key: &str, value: i64) -> Result<(), PylonError> {
        let key = CString::new(key).unwrap();
        if unsafe { !pylon_sys::PylonDeviceFeatureIsWritable(self.handle, key.as_ptr()) } {
            Err(PylonError::with_msg(&format!(
                "Device Feature: {:?} is not writable.",
                key
            )))
        } else {
            checked!(
                pylon_sys::PylonDeviceSetIntegerFeature(self.handle, key.as_ptr(), value),
                ()
            )
        }
    }

    pub fn set_float_feature(&self, key: &str, value: f64) -> Result<(), PylonError> {
        let key = CString::new(key).unwrap();
        if unsafe { !pylon_sys::PylonDeviceFeatureIsWritable(self.handle, key.as_ptr()) } {
            Err(PylonError::with_msg(&format!(
                "Device Feature: {:?} is not writable.",
                key
            )))
        } else {
            checked!(
                pylon_sys::PylonDeviceSetFloatFeature(self.handle, key.as_ptr(), value),
                ()
            )
        }
    }

    pub fn get_string_feature(&self, key: &str) -> Result<String, PylonError> {
        let key = CString::new(key).unwrap();
        if unsafe { !pylon_sys::PylonDeviceFeatureIsReadable(self.handle, key.as_ptr()) } {
            Err(PylonError::with_msg(&format!(
                "Device Feature: {:?} is not readable.",
                key
            )))
        } else {
            let value = vec![0u8; 256];
            let mut size = value.len();
            checked!(
                pylon_sys::PylonDeviceFeatureToString(
                    self.handle,
                    key.as_ptr(),
                    value.as_ptr() as *mut i8,
                    &mut size,
                ),
                String::from_utf8(value).unwrap()
            )
        }
    }

    pub fn get_integer_feature(&self, key: &str) -> Result<i64, PylonError> {
        let key = CString::new(key).unwrap();
        if unsafe { !pylon_sys::PylonDeviceFeatureIsReadable(self.handle, key.as_ptr()) } {
            Err(PylonError::with_msg(&format!(
                "Device Feature: {:?} is not readable.",
                key
            )))
        } else {
            let mut value = 0;
            checked!(
                pylon_sys::PylonDeviceGetIntegerFeature(self.handle, key.as_ptr(), &mut value),
                value
            )
        }
    }

    pub fn grab_single_frame(&mut self) -> Result<DynamicImage, PylonError> {
        let mut buffer_ready = false;
        let mut grab_result = pylon_sys::PylonGrabResult_t::default();
        checked!(
            pylon_sys::PylonDeviceGrabSingleFrame(
                self.handle,
                0,
                self.grab_buffer.as_mut_ptr() as *mut c_void,
                self.grab_buffer.len(),
                &mut grab_result,
                &mut buffer_ready,
                500,
            ),
            ()
        )?;

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
    use std::env;

    #[test]
    fn grab_frame() {
        // emulate one camera with testpattern
        env::set_var("PYLON_CAMEMU", "1");
        super::initialize();
        let mut dev = super::Device::create_device_by_index(0).unwrap();
        dev.open().unwrap();
        let frame = dev.grab_single_frame().unwrap().to_luma();

        // check pixel content of shifted ramp testimage
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                println!("{}, {}", x, y);
                assert_eq!((x + y) % 256, frame.get_pixel(x, y)[0] as u32);
            }
        }
    }
}

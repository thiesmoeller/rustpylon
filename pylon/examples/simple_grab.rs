use pylon;
use std::error::Error;

fn main() -> Result<(), Box<Error>> {
  pylon::initialize();
  let devices = pylon::Device::enumerate_devices()?;
  println!("Number of devices: {}", devices);

  let mut dev = pylon::Device::create_device_by_index(0)?;
  dev.open()?;

  let s = dev.get_string_feature("DeviceModelName")?;
  println!("Device: {}", s);

  let frame = dev.grab_single_frame()?.to_luma();
  println!("Frame: {}x{}", frame.width(), frame.height());

  Ok(())
}

use pylon;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
  pylon::initialize();
  let devices = pylon::Device::enumerate_devices()?;
  println!("Number of devices: {}", devices);

  let mut dev = pylon::Device::create_device_by_index(0)?;
  dev.open()?;

  let s = dev.get_string_feature("DeviceModelName")?;
  println!("Device: {}", s);

  let path = env::temp_dir().join("image.jpg");
  let frame = dev.grab_single_frame()?.to_luma();
  println!("Frame: {}x{}", frame.width(), frame.height());

  println!("Save to: {}", path.to_string_lossy());
  frame.save(&path)?;
  Ok(())
}

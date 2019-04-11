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

  // dev.set_string_feature("AcquisitionMode", "Continuous")?;

  for n in 1..100 {
    let frame = dev.grab_single_frame()?.to_luma();
    println!("Frame: {}x{}", frame.width(), frame.height());
    let path = format!("/tmp/image_{:02}.jpg", n);
    println!("Save to: {}", path);
    frame.save(path)?;
  }
  Ok(())
}

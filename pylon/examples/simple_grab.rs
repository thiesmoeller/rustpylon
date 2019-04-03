use pylon;
use std::error::Error;

fn main() -> Result<(), Box<Error>> {
  pylon::initialize();
  let devices = pylon::Device::enumerate_devices()?;
  println!("Number of devices: {}", devices);
  let dev = pylon::Device::create_device_by_index(0)?;
  println!("{}", dev);
  Ok(())
}

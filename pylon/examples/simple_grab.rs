use pylon;
use std::error::Error;

fn main() -> Result<(), Box<Error>> {
  pylon::initialize();
  let devices = pylon::Device::enumerate_devices()?;
  println!("Number of devices: {}", devices);
  Ok(())
}

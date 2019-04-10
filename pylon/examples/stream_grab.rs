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

  dev.set_string_feature("TriggerSelector", "AcquisitionStart")?;
  dev.set_string_feature("TriggerMode", "Off")?;
  dev.set_string_feature("TriggerSelector", "FrameStart")?;
  dev.set_string_feature("TriggerMode", "Off")?;

  dev.set_string_feature("AcquisitionMode", "Continuous")?;
  let mut stream = dev.get_stream_grabber(0)?;
  dev.execute_command("AcquisitionStart")?;

  for n in 0..100 {
    let img = stream.grab()?;
    let path = format!("/tmp/image_{:02}.jpg", n);
    println!("Save to: {}", path);
    //img.save(path)?;
  }

  dev.execute_command("AcquisitionStop")?;


  Ok(())
}
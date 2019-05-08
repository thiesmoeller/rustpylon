use pylon;
use std::error::Error;
use tokio::prelude::*;
use future::lazy;

fn main() -> Result<(), Box<Error>> {
  pylon::initialize();
  let devices = pylon::Device::enumerate_devices()?;
  println!("Number of devices: {}", devices);

  let mut dev = pylon::Device::create_device_by_index(0)?;
  dev.open()?;

  let s = dev.get_string_feature("DeviceModelName")?;
  println!("Device: {}", s);

  dev.set_string_feature("AcquisitionMode", "Continuous")?;
  dev.set_float_feature("AcquisitionFrameRateAbs", 40.0)?;
  let mut stream = dev.get_stream_grabber(0)?;
  dev.execute_command("AcquisitionStart")?;

  tokio::run(stream.chunks(20).enumerate().for_each(|(n, v)| {
            tokio::spawn(lazy(move|| {
              for (m, i) in v.iter().enumerate() {
                let path = format!("/tmp/image_{:02}-{:02}.jpg", n, m);
                println!("Save to: {}", path);
                i.save(path).unwrap();
              }
              Ok(())
            }));
            Ok(())
        }).map_err(|e| println!("Err {}", e)));

  dev.execute_command("AcquisitionStop")?;
  Ok(())
}

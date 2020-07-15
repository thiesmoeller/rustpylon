use pylon;
use std::error::Error;
use tokio::stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  pylon::initialize();
  let devices = pylon::Device::enumerate_devices()?;
  println!("Number of devices: {}", devices);

  let mut dev = pylon::Device::create_device_by_index(0)?;
  dev.open()?;

  let s = dev.get_string_feature("DeviceModelName")?;
  println!("Device: {}", s);

  dev.set_string_feature("AcquisitionMode", "Continuous")?;
  dev.set_float_feature("AcquisitionFrameRateAbs", 50.0)?;
  let stream = dev.get_stream_grabber(0)?;
  dev.execute_command("AcquisitionStart")?;

  stream
    .take(100)
    .fold(0, |acc, img| {
      tokio::task::spawn_blocking(move || {
        println!("{:?}", std::thread::current());
        let path = format!("/tmp/image_{:02}.jpg", acc);
        println!("Save to: {}", path);
        let out = image::imageops::blur(&img.unwrap(), 20.0);
        out.save(path).unwrap();
      });
      acc + 1
    })
    .await;

  dev.execute_command("AcquisitionStop")?;

  Ok(())
}

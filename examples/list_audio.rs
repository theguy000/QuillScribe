use cpal::traits::{DeviceTrait, HostTrait};
fn main() {
    let host = cpal::default_host();
    let devices = host.input_devices().unwrap();
    for d in devices {
        println!("{}", d.name().unwrap());
    }
}

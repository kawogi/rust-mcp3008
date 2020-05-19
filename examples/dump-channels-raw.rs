#![deny(clippy::all)]

use mcp3208::{Mcp3208, Channel};

/// outputs the raw adc values of all channels
fn main() {
    if let Ok(mut mcp3208) = Mcp3208::new("/dev/spidev0.0") {
        Channel::VALUES.iter().for_each(|&channel| {
            println!("channel #{}: {}", channel as u8, mcp3208.read_adc_single(channel).unwrap());
        });
    }
}
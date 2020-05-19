#![deny(clippy::all)]

use mcp3208::Mcp3208;

/// outputs the raw adc values of all channels
fn main() {
    if let Ok(mut mcp3208) = Mcp3208::new("/dev/spidev0.0") {
        for channel_id in 0..8 {
            println!("channel #{}: {}", channel_id, mcp3208.read_adc_single(channel_id).unwrap());
        }
    }
}
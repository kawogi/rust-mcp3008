#![deny(clippy::all)]

use mcp3208::Mcp3208;

fn main() {
    if let Ok(mut mcp3208) = Mcp3208::new("/dev/spidev0.0") {
        for channel_id in 0..8 {
            println!("{}", mcp3208.read_adc(channel_id).unwrap());
        }
    }
}
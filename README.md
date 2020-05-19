# rust-mcp3208

[![](https://img.shields.io/crates/v/mcp3208.svg)](https://crates.io/crates/mcp3208)
[![](https://docs.rs/mcp3208/badge.svg)](https://docs.rs/mcp3208)

[![MCP3208 A/D converter](https://www.microchip.com/_images/products/medium/4a2eee4577eb56184dce8b01c5556be9.png "MCP3208 A/D converter")](https://www.microchip.com/wwwproducts/en/MCP3208)

`rust-mcp3208` is a library to read adc values from an MCP3208 via spi. 

## Usage
<details>
<summary>
Cargo.toml
</summary>

```toml
[dependencies]
mcp3208 = "1.0.0"
```

</details>

```rust
use mcp3208::{Mcp3208, Channel};

/// outputs the raw adc values of all channels
fn main() {
    if let Ok(mut mcp3208) = Mcp3208::new("/dev/spidev0.0") {
        Channel::VALUES.iter().for_each(|&channel| {
            println!("channel #{}: {}", channel as u8, mcp3208.read_adc_single(channel).unwrap());
        });
    }
}
```

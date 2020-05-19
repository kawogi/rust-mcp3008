# rust-mcp3208

[![](https://img.shields.io/crates/v/mcp3208.svg)](https://crates.io/crates/mcp3208)
[![](https://docs.rs/mcp3208/badge.svg)](https://docs.rs/mcp3208)

<p align="center">
	<img src="https://user-images.githubusercontent.com/9287847/35982700-b8731460-0cf0-11e8-836a-42537d76396e.png" height="300"/>
</p>

<p align="center">
	<strong>MCP3208 A/D converter</strong>
</p>


`rust-mcp3208` is a rewrite of the excellent [Adafruit_Python_MCP3008](https://github.com/adafruit/Adafruit_Python_MCP3008) Python library in Rust. 

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

<p></p>

```rust
extern crate mcp3208;

use mcp3208::Mcp3208;

fn main() {
    if let Ok(mut mcp3208) = Mcp3208::new("/dev/spidev0.0") {
        println!("{}", mcp3208.read_adc(0).unwrap());
    }
}
```

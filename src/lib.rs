//! `rust-mcp3208` is a rewrite of the excellent [Adafruit_Python_MCP3008](https://github.com/adafruit/Adafruit_Python_MCP3008) Python library in Rust.

#[cfg(test)]
mod tests {
    use super::Mcp3208;
    use std::path::Path;
    use std::env;

    #[test]
    fn mcp3208_read_adc() {
        let spi_dev_path = "/dev/spidev0.0";

        if cfg!(target_os = "linux") {
            if Path::new(&spi_dev_path).exists() {
                let mut mcp3208 = Mcp3208::new(spi_dev_path).unwrap();

                mcp3208.read_adc(0).unwrap();

                if let Ok(_) = mcp3208.read_adc(8) {
                    panic!("read from adc > 7");
                }
            } else {
                if let Ok(_) = env::var("TRAVIS_RUST_VERSION") {
                    println!("can't mock spi interface on travis, passing test...");
                } else {
                    panic!("can not test on current setup (no spi interface)");
                }
            }
        } else {
            panic!("can not test on current setup (unsupported os)");
        }
    }
}

#[cfg(target_os = "linux")]
extern crate spidev;

use std::io;
use std::fmt;
use std::error::Error;

const RESOLUTION: u8 = 12;
const CHANNEL_SELECT_BITS: u8 = 3;
const SAMPLE_DELAY_BITS: u8 = 1;
const SAMPLE_ZERO_BITS: u8 = 1;
const CHECKSUMMED_RESULT_BITS: u8 = RESOLUTION - 1 + RESOLUTION;

const CHANNEL_COUNT: u8 = 1u8 << CHANNEL_SELECT_BITS;
const CHANNEL_MASK: u8 = CHANNEL_COUNT - 1;

#[cfg(target_os = "linux")]
use spidev::{SPI_MODE_0, Spidev, SpidevOptions, SpidevTransfer};

#[derive(Debug)]
pub enum Mcp3208Error {
    SpidevError(io::Error),
    AdcOutOfRangeError(u8),
    UnsupportedOSError,
    DataError(String),
}

impl fmt::Display for Mcp3208Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mcp3208Error::SpidevError(ref err) => err.fmt(f),
            Mcp3208Error::AdcOutOfRangeError(adc_number) => {
                write!(f, "invalid adc number ({})", adc_number)
            }
            Mcp3208Error::UnsupportedOSError => write!(f, "unsupported os"),
            Mcp3208Error::DataError(ref message) => f.write_str(message),
        }
    }
}

impl Error for Mcp3208Error {

    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            Mcp3208Error::SpidevError(ref err) => Some(err),
            Mcp3208Error::AdcOutOfRangeError(_) => None,
            Mcp3208Error::UnsupportedOSError => None,
            Mcp3208Error::DataError(_) => None,
        }
    }
}

impl From<io::Error> for Mcp3208Error {
    fn from(err: io::Error) -> Mcp3208Error {
        Mcp3208Error::SpidevError(err)
    }
}

pub struct Mcp3208 {
    #[cfg(target_os = "linux")]
    spi: Spidev,
}

/// Provides access to a MCP3208 A/D converter.
/// # Example
///
/// ```rust
/// extern crate mcp3208;
///
/// use mcp3208::Mcp3208;
///
/// fn main() {
///     if let Ok(mut mcp3208) = Mcp3208::new("/dev/spidev0.0") {
///         println!("{}", mcp3208.read_adc(0).unwrap());
///     }
/// }
/// ```
impl Mcp3208 {
    /// Constructs a new `Mcp3208`.
    #[cfg(target_os = "linux")]
    pub fn new(spi_dev_path: &str) -> Result<Mcp3208, Mcp3208Error> {
        let options = SpidevOptions::new()
            .max_speed_hz(1_000_000)
            .mode(SPI_MODE_0)
            .lsb_first(false)
            .build();

        let mut spi = Spidev::open(spi_dev_path.to_string())?;

        match spi.configure(&options) {
            Ok(_) => Ok(Mcp3208 { spi }),
            Err(err) => Err(Mcp3208Error::SpidevError(err)),
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new(_spi_dev_path: &str) -> Result<Mcp3208, Mcp3208Error> {
        Err(Mcp3208Error::UnsupportedOSError)
    }

    #[cfg(target_os = "linux")]
    pub fn read_adc(&mut self, adc_number: u8) -> Result<u16, Mcp3208Error> {
        match adc_number {
            0..=7 => {
                // Start bit, single channel read
                let mut command: u8 = 0b11 << 6;
                command |= (adc_number & 0x07) << 3;
                // Bottom 3 bits of command are 0, this is to account for the
                // extra clock to do the conversion, and the low null bit returned
                // at the start of the response.

                let is_differential = false;

                // smcccw0r_rrrrrrrr_rrrxxxxx_xxxxxx00
                // s: start bit = 1
                // m: mode bit
                // c: channel selection bit
                // r: response bit (msb first)
                // x: checksum bit (lsb first)

                let start_bits = 1 << 31;
                let mode_bits = if is_differential { 0 } else { 1 }  << 30;
                let channel_selection_bits = (adc_number as u32) << (30 - CHANNEL_SELECT_BITS);
                let command_bits = start_bits | mode_bits | channel_selection_bits;

                let response_bits = self.send_command_bits(command_bits)?;

                // everything except the actual sample values and its checksum has to be zero.
                if response_bits & 0b_11111110_00000000_00000000_00000011 != 0 {
                    return Err(Mcp3208Error::DataError(format!("invalid response: 0x{:04x}", response_bits)));
                }

                let checksum = response_bits.reverse_bits() >> 5;
                if response_bits != checksum {
                    return Err(Mcp3208Error::DataError(format!("invalid checksum: 0x{:04x}", response_bits)));
                }

                Ok((response_bits >> 13) as u16)
            }
            _ => Err(Mcp3208Error::AdcOutOfRangeError(adc_number)),
        }
    }

    #[inline]
    fn send_command_bits(&self, command: u32) -> Result<u32, Mcp3208Error> {
        // split into big endian form
        let tx_buf = [(command >> 24) as u8, (command >> 16) as u8, (command >> 8) as u8, command as u8];
        let mut rx_buf = [0_u8; 4];

        let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
        self.spi.transfer(&mut transfer)?;

        // join from big endian
        Ok((rx_buf[0] as u32) << 24 | (rx_buf[1] as u32) << 16 | (rx_buf[2] as u32) << 8 | (rx_buf[3] as u32))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn read_adc(&mut self, _adc_number: u8) -> Result<u16, Mcp3208Error> {
        Err(Mcp3208Error::UnsupportedOSError)
    }
}

//! `rust-mcp3208` is a rewrite of the excellent [Adafruit_Python_MCP3008](https://github.com/adafruit/Adafruit_Python_MCP3008) Python library in Rust.

#[cfg(test)]
mod tests {
    use super::Mcp3208;
    use std::path::Path;
    use std::env;

    #[test]
    fn mcp3208_read_adc_single() {
        let spi_dev_path = "/dev/spidev0.0";

        if cfg!(target_os = "linux") {
            if Path::new(&spi_dev_path).exists() {
                let mut mcp3208 = Mcp3208::new(spi_dev_path).unwrap();

                mcp3208.read_adc_single(0).unwrap();

                if let Ok(_) = mcp3208.read_adc_single(8) {
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

/// Number of bits to be sent/received within a single transaction
const FRAME_BIT_COUNT: u8 = 32;

/// number of start bits (always 1)
const START_BIT_COUNT: u8 = 1;

/// index of first (and only) start bit (always the MSB bit)
const START_BIT_INDEX: u8 = FRAME_BIT_COUNT - START_BIT_COUNT; // 31

/// number of bits to select the mode (single or differential)
const MODE_BIT_COUNT: u8 = 1;

/// index of the first (and only) mode selection bit
const MODE_BIT_INDEX: u8 = START_BIT_INDEX - MODE_BIT_COUNT; // 30

/// number of bits required to encode the selected channel
const CHANNEL_BIT_COUNT: u8 = 3;

/// index of the first bit of the channel selection field
const CHANNEL_BITS_INDEX: u8 = MODE_BIT_INDEX - CHANNEL_BIT_COUNT; // 27

/// number of supported channels
const CHANNEL_COUNT: u8 = 1 << CHANNEL_BIT_COUNT;

/// number of bits to wait for the response (always 1)
const WAIT_BIT_COUNT: u8 = 1;

/// index of the first (and only) wait bit
const WAIT_BIT_INDEX: u8 = CHANNEL_BITS_INDEX - WAIT_BIT_COUNT; // 26

/// number of zero bits before the returned sample value (always 1)
const ZERO_BIT_COUNT: u8 = 1;

/// index of the first (and only) zero bit before the returned sample value
const ZERO_BIT_INDEX: u8 = WAIT_BIT_INDEX - ZERO_BIT_COUNT; // 25

/// resolution of the adc in bits (the MCP3208 has a 12-bit resolution)
const SAMPLE_BIT_COUNT: u8 = 12;

/// position of the first bit (lsb) of the sampled value within the response
const SAMPLE_BITS_INDEX: u8 = ZERO_BIT_INDEX - SAMPLE_BIT_COUNT; // 13

/// number of checksum bits within the response
const CHECKSUM_BIT_COUNT: u8 = SAMPLE_BIT_COUNT - 1; // 11

/// position of the first bit (msb) of the checksum value within the response
const CHECKSUM_BITS_INDEX: u8 = SAMPLE_BITS_INDEX - CHECKSUM_BIT_COUNT; // 2

/// number of trailing zero-bits within the response
const PADDING_BIT_COUNT: u8 = CHECKSUM_BITS_INDEX; // 2

/// index of the trailing zero-bits within the response (always 0)
const PADDING_BITS_INDEX: u8 = 0;

/// index of the lsb of the sampled value _before_ reversing the bits for validation
const SAMPLE_LSB_INDEX: i8 = SAMPLE_BITS_INDEX as i8; // 13

/// index of the lsb of the sampled value _after_ reversing the bits for validation
const SAMPLE_LSB_MIRRORED_INDEX: i8 = (FRAME_BIT_COUNT - 1) as i8 - SAMPLE_LSB_INDEX; // 18

/// number of bits to move the reversed pattern to the right to make the lsb align with the original bit-order
/// This value might be negative, indicating a left-shift is required.
const SAMPLE_LSB_SHR: i8 = (FRAME_BIT_COUNT - 1) as i8 - 2 * SAMPLE_BITS_INDEX as i8;

/// mask indicating which bits of the response always have to be zero
const ZERO_VALIDATION_MASK: u32 =
        mask(START_BIT_COUNT) << START_BIT_INDEX |
        mask(MODE_BIT_COUNT) << MODE_BIT_INDEX |
        mask(CHANNEL_BIT_COUNT) << CHANNEL_BITS_INDEX |
        mask(WAIT_BIT_COUNT) << WAIT_BIT_INDEX |
        mask(ZERO_BIT_COUNT) << ZERO_BIT_INDEX |
        mask(PADDING_BIT_COUNT) << PADDING_BITS_INDEX;

/// returns a right-aligned bit-mask with `length` bits set to `1`
const fn mask(length: u8) -> u32 {
    (0x0000_0000_ffff_ffff_u64 >> (32 - length)) as u32
}


#[cfg(target_os = "linux")]
use spidev::{SPI_MODE_0, Spidev, SpidevOptions, SpidevTransfer};
use std::cmp::Ordering;
use std::convert::TryFrom;

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Channel {
    Ch0 = 0x0, Ch1, Ch2, Ch3, Ch4, Ch5, Ch6, Ch7
}

impl Channel {
    /// List of all channels to facilitate iteration and selection by integer indices
    pub const VALUES: [Channel; 8] = [
            Channel::Ch0, Channel::Ch1, Channel::Ch2, Channel::Ch3,
            Channel::Ch4, Channel::Ch5, Channel::Ch6, Channel::Ch7
            ];

    /// Return the channel which will be used in pseudo-differential query mode
    pub fn partner(self) -> Self {
        Self::VALUES[(self as u8 ^ 0b001) as usize]
    }
}

/// Try to convert an integer into a typed channel
impl TryFrom<u8> for Channel {
    type Error = Mcp3208Error;

    fn try_from(channel_index: u8) -> Result<Self, Self::Error> {
        if channel_index < CHANNEL_COUNT {
            Ok(Channel::VALUES[channel_index as usize])
        } else {
            Err(Mcp3208Error::AdcOutOfRangeError(channel_index))
        }
    }
}

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
///         println!("{}", mcp3208.read_adc_single(0).unwrap());
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

    /// read a raw adc value from a selected channel in single-ended mode
    #[cfg(target_os = "linux")]
    pub fn read_adc_single(&mut self, channel: Channel) -> Result<u16, Mcp3208Error> {
        let request = Self::build_request(false, channel);
        let response = self.send_request(request)?;
        Self::parse_response(response)
    }

    /// read a raw adc value from a selected channel in pseudo-differential mode
    #[cfg(target_os = "linux")]
    pub fn read_adc_diff(&mut self, channel: Channel) -> Result<u16, Mcp3208Error> {
        let request = Self::build_request(true, channel);
        let response = self.send_request(request)?;
        Self::parse_response(response)
    }

    #[inline]
    fn build_request(is_differential: bool, channel: Channel) -> u32 {
        // pattern:
        //   smcccw0r_rrrrrrrr_rrrxxxxx_xxxxxx00
        // request:
        //   s: start bit = 1
        //   m: mode bit
        //   c: channel selection bit
        // response:
        //   r: response bit (msb first)
        //   x: checksum bit (lsb first)

        let start_bits = 1u32 << START_BIT_INDEX;
        let mode_bits = if is_differential { 0u32 } else { 1u32 }  << MODE_BIT_INDEX;
        let channel_selection_bits = (channel as u32) << CHANNEL_BITS_INDEX;
        start_bits | mode_bits | channel_selection_bits
    }

    #[inline]
    fn send_request(&self, request: u32) -> Result<u32, Mcp3208Error> {
        let tx_buf = request.to_be_bytes();
        let mut rx_buf = [0_u8; 4];

        let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
        self.spi.transfer(&mut transfer)?;

        Ok(u32::from_be_bytes(rx_buf))
    }

    #[cfg(not(target_os = "linux"))]
    fn send_request(&self, command: u32) -> Result<u32, Mcp3208Error> {
        Err(Mcp3208Error::UnsupportedOSError)
    }

    #[inline]
    fn parse_response(response_bits: u32) -> Result<u16, Mcp3208Error> {
        // pattern:
        //   smcccw0r_rrrrrrrr_rrrxxxxx_xxxxxx00
        // request:
        //   s: start bit = 1
        //   m: mode bit
        //   c: channel selection bit
        // response:
        //   r: response bit (msb first)
        //   x: checksum bit (lsb first)

        // everything except the actual sample value and its checksum has to be zero.
        if response_bits & ZERO_VALIDATION_MASK != 0 {
            return Err(Mcp3208Error::DataError(format!("invalid response: 0x{:04x}", response_bits)));
        }

        // check if the sampled value is followed by a bit-mirrored copy
        let reversed = response_bits.reverse_bits();
        // align original value and mirrored copy
        let checksum = match i8::cmp(&SAMPLE_LSB_INDEX, &SAMPLE_LSB_MIRRORED_INDEX) {
            Ordering::Less => reversed >> SAMPLE_LSB_SHR.abs(),
            Ordering::Equal => reversed,
            Ordering::Greater => reversed >> SAMPLE_LSB_SHR.abs(),
        };

        if response_bits != checksum {
            return Err(Mcp3208Error::DataError(format!("invalid checksum: 0x{:04x}", response_bits)));
        }

        Ok((response_bits >> SAMPLE_BITS_INDEX) as u16)
    }

}

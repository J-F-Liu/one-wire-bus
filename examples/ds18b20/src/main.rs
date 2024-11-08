#![no_std]
#![no_main]

use core::convert::Infallible;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    gpio::{Flex, Pin, Pull, Speed},
    Peripheral,
};
use embassy_time::{Delay, Timer};
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};
use one_wire_bus::OneWire;
use {defmt_rtt as _, panic_probe as _};

pub struct InoutPin<'d, T: Pin> {
    pin: Flex<'d, T>,
    is_push_pull: bool,
    is_input: bool,
}

impl<'d, T: Pin> InoutPin<'d, T> {
    pub fn new(pin: impl Peripheral<P = T> + 'd, is_push_pull: bool) -> Self {
        let mut flex = Flex::new(pin);
        flex.set_as_output(Speed::High);
        Self {
            pin: flex,
            is_push_pull,
            is_input: false,
        }
    }

    pub fn set_as_input(&mut self) {
        if !self.is_input {
            self.pin.set_as_input(Pull::None);
            self.is_input = true;
        }
    }

    pub fn set_as_output(&mut self) {
        if self.is_input {
            if self.is_push_pull {
                self.pin.set_as_output(Speed::High);
            } else {
                self.pin.set_as_input_output(Speed::High, Pull::None);
            }
            self.is_input = false;
        }
    }
}

impl<'d, T: Pin> ErrorType for InoutPin<'d, T> {
    type Error = Infallible;
}

impl<'d, T: Pin> InputPin for InoutPin<'d, T> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.set_as_input();
        Ok(self.pin.is_high())
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.set_as_input();
        Ok(self.pin.is_low())
    }
}

impl<'d, T: Pin> OutputPin for InoutPin<'d, T> {
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_as_output();
        Ok(self.pin.set_high())
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set_as_output();
        Ok(self.pin.set_low())
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("Read Temperature");

    // Which pin to use depends on the circuit board
    let pin = p.PE3;

    // If the DS18B20 is powered from an external VCC, use push pull mode, otherwise use open drain mode.
    let one_wire_pin = InoutPin::new(pin, true);

    let mut delay = Delay;
    let mut sensor = OneWire::new(one_wire_pin).expect("Pin should be high during idle");

    loop {
        let temp = read_temperature(&mut sensor, &mut delay);
        info!("Temperature: {}°C", temp);
        Timer::after_millis(500).await;
    }
}

use embedded_hal::delay::DelayNs;

/// Read temperature from a single DS18B20 sensor.
/// If there are multiple sensors on the bus, a more complex procedure is required.
fn read_temperature<'d, T: Pin>(
    sensor: &mut OneWire<InoutPin<'d, T>>,
    delay: &mut impl DelayNs,
) -> f32 {
    // send convert temperature command
    sensor.send_command(0x44, None, delay).unwrap();

    // wait for conversion to complete
    delay.delay_ms(750);

    // send read scratchpad command
    sensor.send_command(0xbe, None, delay).unwrap();

    // read scratchpad
    let mut scratchpad = [0u8; 2];
    sensor.read_bytes(&mut scratchpad, delay).unwrap();

    // skip reading other bytes
    sensor.reset(delay).unwrap();

    // convert temperature
    let mut temperature = i16::from_le_bytes([scratchpad[0], scratchpad[1]]) as f32;
    temperature = temperature / 16.0; // defaut resolution is 12-bit

    temperature
}

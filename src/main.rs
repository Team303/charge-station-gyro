#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

mod millis;

use core::panic;

use arduino_hal::hal::port::PB2;
use arduino_hal::prelude::*;
use arduino_hal::spi;
use arduino_hal::spi::ChipSelectPin;
use arduino_hal::spi::DataOrder;
use arduino_hal::spi::SerialClockRate;
use arduino_hal::Spi;
use embedded_hal::digital::v2::OutputPin;
use embedded_hal::spi::FullDuplex;
use embedded_hal::spi::MODE_0;
use panic_halt as _;

const SAMPLE_PERIOD: u16 = 2;
const CALIBRATION_SAMPLE_TIME: u16 = 5_000;
const DEGREE_PER_SECOND_PER_LSB: f32 = 0.0125;

/**
 * Represents the registers in the device's memory
 *
 * Each register has a low and high address (n + 1), and some like CST span a total of 4 addresses
 */
#[allow(dead_code)]
enum Register {
    Rate,
    Temp,
    ContinuousSelfTestLow,
    ContinuousSelfTestHigh,
    Quad,
    Fault,
    PartID,
    SerialNumberHigh,
    SerialNumberLow,
}

impl Register {
    fn get_address(&self) -> u16 {
        match self {
            Register::Rate => 0x00,
            Register::Temp => 0x02,
            Register::ContinuousSelfTestLow => 0x04,
            Register::ContinuousSelfTestHigh => 0x06,
            Register::Quad => 0x08,
            Register::Fault => 0x0A,
            Register::PartID => 0x0C,
            Register::SerialNumberHigh => 0x0E,
            Register::SerialNumberLow => 0x10,
        }
    }
}

#[arduino_hal::entry]
fn setup() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    /* Pin definitions */

    let sclk = pins.d13.into_output();
    let mosi = pins.d11.into_output();
    let miso = pins.d12.into_pull_up_input();
    let cs0 = pins.d10.into_output();

    let reset_pin = pins.d5.into_pull_up_input();

    // Set up serial interface for text output
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);

    // Setup millisecond interrupt
    millis::millis_init(dp.TC0);

    // Enable interrupts globally
    unsafe { avr_device::interrupt::enable() };

    // Create SPI interface.
    let (spi, cs) = arduino_hal::Spi::new(
        dp.SPI,
        sclk,
        mosi,
        miso,
        cs0,
        spi::Settings {
            data_order: DataOrder::MostSignificantFirst,
            clock: SerialClockRate::OscfOver128,
            mode: MODE_0,
        },
    );

    // Create gyro instance
    let mut gyro = match ADXRS450::new(spi, cs) {
        Ok(gyro) => gyro,
        Err(e) => panic!("{}", e.as_str()),
    };

    loop {
        // If reset switch is pulled low (closed), reset the gyro
        if reset_pin.is_low() {
            gyro.reset()
        }

        // Update the gyro accumulator
        gyro.update();

        // Print out gyro state
        ufmt::uwriteln!(&mut serial, "Gyro Rate: {}°/s | Gyro Angle: {}°\r", gyro.get_rate() as u32, gyro.get_angle() as u32).void_unwrap();

        // Wait before continuing (trying to get 500Hz)
        arduino_hal::delay_ms(SAMPLE_PERIOD);
    }}

struct ADXRS450 {
    spi: Spi,
    cs: ChipSelectPin<PB2>,
    acc: AccumulatorF32,
}

enum ADXRS450Error {
    DeviceNotFound(u16),
}

impl ADXRS450Error {
    fn as_str(&self) -> &'static str {
        match &self {
            ADXRS450Error::DeviceNotFound(_) => "could not find ADXRS450 gyro on SPI port",
        }
    }
}

impl ADXRS450 {
    fn new(spi: Spi, cs: ChipSelectPin<PB2>) -> Result<Self, ADXRS450Error> {
        let mut gyro = ADXRS450 {
            spi,
            cs,
            acc: AccumulatorF32::new(),
        };

        /* Validate the part ID */

        let part_id = gyro.read_register(Register::PartID);

        // Lower byte is the revision number, so only check that the high byte is 0x52
        if (part_id & 0xff00) != 0x5200 {
            return Err(ADXRS450Error::DeviceNotFound(part_id));
        }

        gyro.calibrate();

        Ok(gyro)
    }

    pub fn read_register(&mut self, register: Register) -> u16 {
        /*
        | Read Command:
        |
        | 00000000 00000000 00000000 00000000
        | cccsssaa aaaaaaad dddddddd dddddddp
        | ^                                 ^
        | MSB                             LSB

        | 31-29 (c) bits are the read command instruction (0b100)
        | 28-26 (s) are SM2-SM0 (all 0s for the ADXRS450)
        | 25-17 (a) are are the 9 bit register address A8-A0
        | 16-1 (d) are the 16 data bits D15-D0 (all 0s for the read command)
        | 0 (p) is the parity bit
        */

        let mut command: u32 = 0;

        // Read instruction
        command |= 0b10000000_00000000_00000000_00000000;

        // Address register
        command |= (register.get_address() as u32) << 17;

        // Parity bit
        command |= calculate_parity(command) as u32;

        self.cs.set_low().unwrap();

        nb::block!(self.spi.send((command & 0xFF000000 >> 24) as u8)).void_unwrap();
        nb::block!(self.spi.send((command & 0x00FF0000 >> 16) as u8)).void_unwrap();
        nb::block!(self.spi.send((command & 0x0000FF00 >> 8) as u8)).void_unwrap();
        nb::block!(self.spi.send((command & 0x000000FF >> 0) as u8)).void_unwrap();

        self.cs.set_high().unwrap();
        self.cs.set_low().unwrap();

        let data_0 = nb::block!(self.spi.read()).void_unwrap();
        let data_1 = nb::block!(self.spi.read()).void_unwrap();
        let data_2 = nb::block!(self.spi.read()).void_unwrap();
        let data_3 = nb::block!(self.spi.read()).void_unwrap();

        self.cs.set_high().unwrap();

        // Check if 3 MSB are all 0 (Error Returned)
        if (data_0 & 0b1110_0000) == 0 {
            return 0;
        }

        // TODO: Check response parity bits

        let response = u32::from_be_bytes([data_0, data_1, data_2, data_3]);

        // Extract the 16 data bits and shift them down to a u16
        (response & 0b00000000_00011111_11111111_11100000 >> 5) as u16
    }

    pub fn update(&mut self) {
        let rate = self.read_register(Register::Rate);
        let rate = i16::from_be_bytes([(rate & 0xFF00 >> 8) as u8, (rate & 0x00FF >> 0) as u8]);
        let rate = rate as f32 / 80.0f32;

        self.acc.add_data(rate);
    }

    pub fn calibrate(&mut self) {
        arduino_hal::delay_ms(100);

        self.acc.set_integrated_center(0.0);
        self.acc.reset();

        arduino_hal::delay_ms(CALIBRATION_SAMPLE_TIME);

        let average = self.acc.get_integrated_average();

        self.acc.set_integrated_center(average);
        self.acc.reset();
    }

    pub fn reset(&mut self) {
        self.acc.reset()
    }

    pub fn get_angle(&self) -> f32 {
        self.acc.get_integrated_value() * DEGREE_PER_SECOND_PER_LSB
    }

    pub fn get_rate(&self) -> f32 {
        self.acc.get_last_value() * DEGREE_PER_SECOND_PER_LSB
    }
}

fn calculate_parity(mut value: u32) -> bool {
    let mut parity = false;

    while value != 0 {
        parity = !parity;
        value = value & (value - 1);
    }

    parity
}

struct AccumulatorF32 {
    accumulated: f32,
    samples: u32,
    last_value: f32,
    last_time: u32,
    integrated_center: f32,
}

impl AccumulatorF32 {
    pub fn new() -> Self {
        AccumulatorF32::with_default(0.0)
    }

    pub fn with_default(default: f32) -> Self {
        AccumulatorF32 {
            accumulated: default,
            samples: 0,
            last_value: 0.0,
            last_time: millis::get_millis(),
            integrated_center: 0.0,
        }
    }

    /**
     * Integrate the added data using the trapezoidal method
     */
    pub fn add_data(&mut self, value: f32) {
        let time = millis::get_millis();

        let delta_time = time - self.last_time;
        let area =
            delta_time as f32 * 1e-3 * (self.last_value + value) / 2.0 - self.integrated_center;

        self.accumulated += area;
        self.last_value = value;
        self.last_time = time;
        self.samples += 1;
    }

    pub fn get_integrated_value(&self) -> f32 {
        self.accumulated
    }

    pub fn get_last_value(&self) -> f32 {
        self.last_value
    }

    pub fn reset(&mut self) {
        self.accumulated = 0.0;
        self.last_value = 0.0;
        self.last_time = millis::get_millis();
    }

    pub fn set_integrated_center(&mut self, center: f32) {
        self.integrated_center = center
    }

    pub fn get_integrated_average(&self) -> f32 {
        self.accumulated / self.samples as f32
    }
}

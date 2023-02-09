use arduino_hal::{Spi, spi::ChipSelectPin, hal::port::PB2, prelude::_embedded_hal_blocking_spi_Transfer};
use embedded_hal::digital::v2::OutputPin;

use crate::{accumulator::AccumulatorF32, serial_println, millis};

pub const SAMPLE_PERIOD: u16 = 2;
const CALIBRATION_SAMPLE_TIME: u32 = 5_000;
const DEGREE_PER_SECOND_PER_LSB: f32 = 1.0 / 80.0;

pub struct ADXRS450 {
    spi: Spi,
    cs: ChipSelectPin<PB2>,
    acc: AccumulatorF32,
}

impl ADXRS450 {
    pub fn new(spi: Spi, cs: ChipSelectPin<PB2>) -> Self {
        let mut gyro = ADXRS450 {
            spi,
            cs,
            acc: AccumulatorF32::new(),
        };

        gyro.calibrate();

        gyro
    }

    fn read_sensor_data(&mut self) -> u16 {
        // Begin Write

        self.cs.set_low().unwrap();

        self.spi.transfer(&mut [0x20, 0x00, 0x00, 0x00]).unwrap();

        self.cs.set_high().unwrap();

        // End Write

        arduino_hal::delay_us(500);

        // Begin Read

        self.cs.set_low().unwrap();

        let mut data = [0; 4];
        self.spi.transfer(&mut data).unwrap();

        self.cs.set_high().unwrap();

        // End Read

        let response = u32::from_be_bytes(data);

        // Check if status bits are not 0b01 (Error Returned)
        if ((response >> 24 & 0b0000_1100) >> 2) != 0b01 {
            serial_println!("[?] read_sensor_data() produced an error! ");
            return 0;
        }

        // TODO: Check response parity bits

        // Extract the 16 data bits and shift them down to a u16
        ((response & 0b00000011_11111111_11111100_00000000) >> 10) as u16
    }

    pub fn update(&mut self) {
        let rate = self.read_sensor_data();
        let rate = i16::from_be_bytes(rate.to_be_bytes());

        self.acc.add_data(rate as f32);
    }

    pub fn calibrate(&mut self) {
        serial_println!("[+] Starting calibration...");

        arduino_hal::delay_ms(100);

        self.acc.set_integrated_center(0.0);
        self.acc.reset();

        let start_time = millis::get_millis();

        loop {
            if millis::get_millis() - start_time > CALIBRATION_SAMPLE_TIME {
                break;
            }

            // Update the gyro accumulator
            self.update();

            // Wait before continuing (trying to get 500Hz)
            arduino_hal::delay_ms(SAMPLE_PERIOD);
        }

        let average = self.acc.get_integrated_average();

        self.acc.set_integrated_center(average);
        self.acc.reset();

        serial_println!("[+] Finished calibration!");
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

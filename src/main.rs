#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

mod millis;

use arduino_hal::hal::port::PB2;
use arduino_hal::prelude::*;
use arduino_hal::spi::ChipSelectPin;
use arduino_hal::spi::DataOrder;
use arduino_hal::spi::SerialClockRate;
use arduino_hal::Spi;
use embedded_hal::digital::v2::OutputPin;
use embedded_hal::spi::MODE_0;
use panic_halt as _;

const SAMPLE_PERIOD: u16 = 2;
const CALIBRATION_SAMPLE_TIME: u32 = 5_000;
const DEGREE_PER_SECOND_PER_LSB: f32 = 1.0 / 80.0;

pub mod serial {
    use avr_device::interrupt::Mutex;
    use core::cell::RefCell;

    pub type Usart = arduino_hal::hal::usart::Usart0<arduino_hal::DefaultClock>;
    pub static GLOBAL_SERIAL: Mutex<RefCell<Option<Usart>>> = Mutex::new(RefCell::new(None));

    pub fn init(serial: Usart) {
        avr_device::interrupt::free(|cs| {
            GLOBAL_SERIAL.borrow(cs).replace(Some(serial));
        })
    }

    #[macro_export]
    macro_rules! serial_println {
        ($($arg:tt)*) => {
            ::avr_device::interrupt::free(|cs| {
                if let Some(serial) = &mut *crate::serial::GLOBAL_SERIAL.borrow(cs).borrow_mut() {
                    ::ufmt::uwriteln!(serial, $($arg)*)
                } else {
                    Ok(())
                }
            }).void_unwrap()
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
    let cs0 = pins.d10.into_output_high();

    let reset_pin = pins.d5.into_pull_up_input();

    // Set up serial interface for text output
    let serial = arduino_hal::default_serial!(dp, pins, 57600);

    serial::init(serial);

    serial_println!("[+] Initializing `millis` interrupt");
    // Setup millisecond interrupt
    millis::millis_init(dp.TC0);

    serial_println!("[+] Enabling global interrupt");

    // Enable interrupts globally
    unsafe { avr_device::interrupt::enable() };

    serial_println!("[+] Creating SPI interface");

    // Create SPI interface.
    let (spi, cs) = arduino_hal::Spi::new(
        dp.SPI,
        sclk,
        mosi,
        miso,
        cs0,
        arduino_hal::spi::Settings {
            data_order: DataOrder::MostSignificantFirst,
            clock: SerialClockRate::OscfOver128,
            mode: MODE_0,
        },
    );

    serial_println!("[+] Creating gyro instance");

    // Create gyro instance
    let mut gyro = ADXRS450::new(spi, cs);

    serial_println!("[+] Entering main loop");

    loop {
        // If reset switch is pulled low (closed), reset the gyro
        if reset_pin.is_low() {
            gyro.reset()
        }

        // Update the gyro accumulator
        gyro.update();

        // Print out gyro state
        serial_println!(
            "[?] Gyro Rate: {:?}°/s | Gyro Angle: {:?}°\r",
            gyro.get_rate() as i32,
            gyro.get_angle() as i32
        );

        // Wait before continuing (trying to get 500Hz)
        arduino_hal::delay_ms(SAMPLE_PERIOD);
    }
}

struct ADXRS450 {
    spi: Spi,
    cs: ChipSelectPin<PB2>,
    acc: AccumulatorF32,
}

impl ADXRS450 {
    fn new(spi: Spi, cs: ChipSelectPin<PB2>) -> Self {
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

        let delta_time_ms = time - self.last_time;
        let area =
            delta_time_ms as f32 * 1e-3 * (self.last_value + value) / 2.0 - self.integrated_center;

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

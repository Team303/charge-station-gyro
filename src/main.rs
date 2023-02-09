#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

mod accumulator;
pub mod gyro;
pub mod millis;
pub mod serial;

use arduino_hal::spi::DataOrder;
use arduino_hal::spi::SerialClockRate;
use embedded_hal::spi::MODE_0;
use panic_halt as _;

use crate::gyro::ADXRS450;
use crate::gyro::SAMPLE_PERIOD;

#[arduino_hal::entry]
fn setup() -> ! {
    // Get device peripherals from HAL
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    // Define pins
    let sclk = pins.d13.into_output();
    let mosi = pins.d11.into_output();
    let miso = pins.d12.into_pull_up_input();
    let cs0 = pins.d10.into_output_high();

    let reset_pin = pins.d5.into_pull_up_input();

    // Set up serial interface for text output
    let serial = arduino_hal::default_serial!(dp, pins, 57600);
    serial::init(serial);

    // Setup millisecond interrupt
    serial_println!("[+] Initializing `millis` interrupt");
    millis::millis_init(dp.TC0);

    // Enable interrupts globally
    serial_println!("[+] Enabling global interrupt");
    unsafe { avr_device::interrupt::enable() };

    // Create SPI interface.
    serial_println!("[+] Creating SPI interface");
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

    // Create gyro instance
    serial_println!("[+] Creating gyro instance");
    let mut gyro = ADXRS450::new(spi, cs);

    // Main program loop
    serial_println!("[+] Starting main loop");
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

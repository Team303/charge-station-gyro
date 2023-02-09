# Charge Station Gyro

This repository contains code for interacting with a standard ADXRS450 FRC gyroscope over SPI using an Arduino UNO for the purpose of displaying the state of the Charge Station.

Since the ADXRS450 gyro only provides the current angle rate, the rate is integrated over time to accumulate the current angle. This approach obviously has some error and according the to the data sheet, will drift roughly 25°/hour minimum.

## Pins

| Name | Pin Number | Direction | Description                |
| ---- | ---------- | --------- | -------------------------- |
| SCLK | `13`       | OUT       | Serial Clock               |
| MOSI | `11`       | OUT       | Master-Out Slave-In        |
| MISO | `12`       | IN        | Master-In Slave-Out        |
| CS0  | `10`       | OUT       | Chip Select 0 (active low) |
| RST  | `5`        | IN        | Reset switch (active low)  |
| LED  | `6`        | OUT       | LED Data                   |

## Quickstart

You need a nightly Rust compiler for compiling Rust code for AVR. The correct version will be installed automatically due to the `rust-toolchain.toml` file.

Install dependencies:

- Ubuntu
  ```bash
  sudo apt install avr-libc gcc-avr pkg-config avrdude libudev-dev build-essential
  ```
- Macos
  ```bash
  xcode-select --install # if you haven't already done so
  brew tap osx-cross/avr
  brew install avr-gcc avrdude
  ```
- Windows

  Install [Scoop](https://scoop.sh/) using Powershell

  ```PowerShell
  Set-ExecutionPolicy RemoteSigned -Scope CurrentUser # Needed to run a remote script the first time
  irm get.scoop.sh | iex
  ```

  Install avr-gcc and avrdude

  ```
  scoop install avr-gcc
  scoop install avrdude
  ```

  See [Setting up environment](https://github.com/Rahix/avr-hal/wiki/Setting-up-environment) for more information.

Next, install `ravedude`, a tool which seamlessly integrates flashing your board into the usual cargo workflow:

```bash
cargo install ravedude
```

Finally, flash the code to the arduino and run it automatically:

```bash
cargo run
```

## References

- [ADXRS450 Data Sheet](https://www.analog.com/media/en/technical-documentation/data-sheets/ADXRS450.pdf)
- [ADXRS450 WPILib Implementation](https://github.com/wpilibsuite/allwpilib/blob/main/wpilibj/src/main/java/edu/wpi/first/wpilibj/ADXRS450_Gyro.java)
- [SPI (Serial Peripheral Interface)](https://en.wikipedia.org/wiki/Serial_Peripheral_Interface)

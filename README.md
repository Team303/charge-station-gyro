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

## References

- [ADXRS450 Data Sheet](https://www.analog.com/media/en/technical-documentation/data-sheets/ADXRS450.pdf)
- [ADXRS450 WPILib Implementation](https://github.com/wpilibsuite/allwpilib/blob/main/wpilibj/src/main/java/edu/wpi/first/wpilibj/ADXRS450_Gyro.java)
- [SPI (Serial Peripheral Interface)](https://en.wikipedia.org/wiki/Serial_Peripheral_Interface)

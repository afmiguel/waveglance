# Example files (VCD)

These files are not captures from a device. They are made by
`scripts/make-vcd-examples.py` in this repository, for the File → Open
example menu of the viewer. Run the script again to make the same files.

| File | Content |
|---|---|
| `memory-bus.vcd` | A small CPU bus at 10 MHz: reset, write and read cycles. `addr[15:0]`, `data[7:0]` (z when no device drives it, x before a read answers), `beat[2:0]`, and the strobes `wr_n`, `rd_n`. |
| `dac-sine.vcd` | An 8-bit DAC making a 500 Hz sine: the code `code[7:0]`, the `load` strobe and the analog output `vout` after an RC filter. |
| `uart-at.vcd` | A serial (UART) exchange at 115200 baud, 8N1: a host sends `AT\r\n` on `tx`, the device answers `\r\nOK\r\n` on `rx`. |
| `uart-analog.vcd` | A 3.3 V UART line as an oscilloscope sees it (`tx_pin`, in volts): 9600 baud, 8N1, `Hello, FNIRSI!\r\n`, with slow edges and noise. |
| `uart-session.vcd` | Four serial lines: `host_tx` and `dev_tx` (AT commands and answers, 115200 8N1, then a break on `dev_tx`), `gps` (NMEA sentences, 9600 8N1) and `rs232` (inverted, 19200 7E1, one parity error). |
| `i2c-sensor.vcd` | I2C at 100 kHz (`sda`, `scl`): write a register of a sensor at 0x48, read the temperature with a repeated start (0x19 0x80, the last byte with a NACK), and an address nobody answers (0x50). |
| `i2c-analog.vcd` | The temperature read of `i2c-sensor.vcd` as an oscilloscope sees it: `CH1` (SDA) and `CH2` (SCL) in volts, 3.3 V open-drain lines with slow rising edges and noise. |
| `spi-flash.vcd` | SPI at 1 MHz, mode 0 (`cs`, `sck`, `mosi`, `miso`): a flash memory answers the JEDEC ID (0x9F → EF 40 18), a write enable (0x06), and a read of 9 bytes at 0x000100 (`Hello SPI`). |
| `spi-analog.vcd` | SPI without CS as a two-channel oscilloscope sees it: `CH1` (MOSI) and `CH2` (SCLK) in volts, 500 kHz, mode 0: 0x9F, a pause, then 0x03 0x00 0x01 0x00. |

They are part of this project, under the same terms as its source code.

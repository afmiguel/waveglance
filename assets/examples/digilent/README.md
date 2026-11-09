# Example files (Digilent WaveForms)

These files are not captures from a device. They are made by
`scripts/make-digilent-examples.py` in this repository, in the text format
that Digilent WaveForms writes (File → Export of the Oscilloscope and of the
Logic Analyzer, Analog Discovery 3), for the File → Open example menu of the
viewer. Run the script again to make the same files.

| File | Content |
|---|---|
| `scope-rc-filter.csv` | Oscilloscope, 1 MS/s, 8192 samples: a 1 kHz square wave from the Wavegen, 0 to 3.3 V (`Channel 1`), into an RC low-pass filter of 100 µs (`Channel 2`, the capacitor voltage), with a little noise. Trigger on CH1, rising, at 1.65 V. |
| `logic-i2c-uart.csv` | Logic Analyzer, 2 MHz, 8192 samples: a microcontroller reads a temperature sensor at 0x48 over I2C (`SCL`, `SDA`, 100 kHz: register 0, then 0x19 0x80 = 25.5 °C), then prints `T=25.5C` on its UART (`TX`, 115200 baud, 8N1). |

They are part of this project, under the same terms as its source code.

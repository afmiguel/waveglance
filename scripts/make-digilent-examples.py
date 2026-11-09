#!/usr/bin/env python3
"""Make the Digilent WaveForms example files (File -> Open example).

They are not captures from a device: the signals are computed here, and
written in the text format of WaveForms (File -> Export of the Oscilloscope
and of the Logic Analyzer), with the same "#" header lines.

  assets/examples/digilent/scope-rc-filter.csv
      Oscilloscope, Analog Discovery 3: the Wavegen makes a 1 kHz square
      wave, 0 to 3.3 V (CH1), into an RC low-pass filter, 1 kOhm and
      100 nF (tau = 100 us); CH2 is the capacitor voltage, with noise.
  assets/examples/digilent/logic-i2c-uart.csv
      Logic Analyzer, Analog Discovery 3, 2 MHz: a microcontroller reads a
      temperature sensor at 0x48 over I2C (SCL, SDA, 100 kHz: register 0,
      then two bytes 0x19 0x80 = 25.5 C) and prints "T=25.5C" on its UART
      (TX, 115200 baud, 8N1).

Use: python3 scripts/make-digilent-examples.py
"""

import math
import random
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "assets" / "examples" / "digilent"


def scope():
    rate = 1e6  # 1 MS/s
    n = 8192
    t0 = -n // 2 / rate  # trigger in the middle
    tau = 100e-6
    period = 1e-3
    rng = random.Random(3)
    lines = [
        "#Digilent WaveForms Oscilloscope Acquisition",
        "#Device Name: Discovery3",
        "#Serial Number: SN:210415XXXXXX",
        "#Date Time: 2026-11-09 10:00:00.000.000.000",
        "#Sample rate: 1e+06Hz",
        f"#Samples: {n}",
        "#Trigger: Source: Channel 1 Type: Edge Condition: Rising Level: 1.65 V Hyst.: Auto HoldOff: 0 s",
        "#Channel 1: Range: 1 V/div Offset: -1.65 V Attenuation: 1 X",
        "#Channel 2: Range: 1 V/div Offset: -1.65 V Attenuation: 1 X",
        "#Wavegen Channel 1: Running",
        "#Mode: Simple",
        "#Type: Square",
        "#Frequency: 1 kHz",
        "#Amplitude: 1.65 V",
        "#Offset: 1.65 V",
        "#Symmetry: 50 %",
        "#Phase: 0",
        "",
        "Time (s),Channel 1 (V),Channel 2 (V)",
    ]
    # The filter output, step by step (exact for a constant input over dt).
    dt = 1 / rate
    k = math.exp(-dt / tau)
    vc = 0.0
    # Settle first: start from the steady state of the previous periods.
    rows = []
    for i in range(-20000, n):
        t = t0 + i * dt
        # Square: high in the first half of each period, rising edge at t = 0.
        vin = 3.3 if (t % period) < period / 2 else 0.0
        vc = vin + (vc - vin) * k
        if i >= 0:
            ch1 = vin + rng.gauss(0, 0.004)
            ch2 = vc + rng.gauss(0, 0.006)
            rows.append(f"{t:.9g},{ch1:.6f},{ch2:.6f}")
    (OUT / "scope-rc-filter.csv").write_text("\n".join(lines + rows) + "\n")


class Lines:
    """Digital lines sampled at `rate`; each event sets a line from a time on."""

    def __init__(self, rate, n, names, idle):
        self.rate, self.n, self.names = rate, n, names
        self.events = {name: [(-1.0, idle[name])] for name in names}

    def set(self, name, t, v):
        self.events[name].append((t, v))


def logic():
    rate = 2e6
    n = 8192
    t0 = -0.0005
    L = Lines(rate, n, ["TX", "SCL", "SDA"], {"TX": 1, "SCL": 1, "SDA": 1})
    # I2C at 100 kHz: 10 us per bit; SDA changes in the middle of SCL low.
    t = 0.0
    half = 5e-6

    def start(t):
        L.set("SDA", t, 0)
        L.set("SCL", t + half, 0)
        return t + half

    def bit(t, b):
        L.set("SDA", t + half / 2, b)
        L.set("SCL", t + half, 1)
        L.set("SCL", t + 2 * half, 0)
        return t + 2 * half

    def byte(t, value, ack):
        for k in range(7, -1, -1):
            t = bit(t, (value >> k) & 1)
        return bit(t, 0 if ack else 1)

    def repeated_start(t):
        L.set("SDA", t + half / 2, 1)
        L.set("SCL", t + half, 1)
        L.set("SDA", t + 1.5 * half, 0)
        L.set("SCL", t + 2 * half, 0)
        return t + 2 * half

    def stop(t):
        L.set("SDA", t + half / 2, 0)
        L.set("SCL", t + half, 1)
        L.set("SDA", t + 1.5 * half, 1)
        return t + 2 * half

    t = start(t)
    t = byte(t, 0x48 << 1, True)       # address 0x48, write
    t = byte(t, 0x00, True)            # register 0: temperature
    t = repeated_start(t)
    t = byte(t, (0x48 << 1) | 1, True) # address 0x48, read
    t = byte(t, 0x19, True)            # 25
    t = byte(t, 0x80, False)           # .5, NACK: last byte
    t = stop(t)

    # UART 115200 8N1 after the read.
    baud = 115200
    tb = 1 / baud
    t += 50e-6
    for ch in "T=25.5C\r\n".encode():
        L.set("TX", t, 0)  # start bit
        for k in range(8):
            L.set("TX", t + (k + 1) * tb, (ch >> k) & 1)
        L.set("TX", t + 9 * tb, 1)  # stop bit
        t += 10 * tb + 2 * tb  # a short gap between characters

    lines = [
        "#Digilent WaveForms Logic Analyzer Acquisition",
        "#Device Name: Discovery3",
        "#Serial Number: SN:210415XXXXXX",
        "#Date Time: 2026-11-09 10:05:00.000.000.000",
        "#Sample rate: 2e+06Hz",
        f"#Samples: {n}",
        "",
        "Time (s),TX,SCL,SDA",
    ]
    # Sort the events once, then sample.
    for name in L.names:
        L.events[name].sort()
    idx = {name: 0 for name in L.names}
    cur = {name: L.events[name][0][1] for name in L.names}
    for i in range(n):
        ts = t0 + i / rate
        row = [f"{ts:.9g}"]
        for name in L.names:
            ev = L.events[name]
            while idx[name] + 1 < len(ev) and ev[idx[name] + 1][0] <= ts + 1e-12:
                idx[name] += 1
                cur[name] = ev[idx[name]][1]
            row.append(str(cur[name]))
        lines.append(",".join(row))
    (OUT / "logic-i2c-uart.csv").write_text("\n".join(lines) + "\n")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    scope()
    logic()
    print("examples written to", OUT)


if __name__ == "__main__":
    main()

# Example files (VCD)

These files are not captures from a device. They are made by
`scripts/make-vcd-examples.py` in this repository, for the File → Open
example menu of the viewer. Run the script again to make the same files.

| File | Content |
|---|---|
| `memory-bus.vcd` | A small CPU bus at 10 MHz: reset, write and read cycles. `addr[15:0]`, `data[7:0]` (z when no device drives it, x before a read answers), `beat[2:0]`, and the strobes `wr_n`, `rd_n`. |
| `dac-sine.vcd` | An 8-bit DAC making a 500 Hz sine: the code `code[7:0]`, the `load` strobe and the analog output `vout` after an RC filter. |

They are part of this project, under the same terms as its source code.

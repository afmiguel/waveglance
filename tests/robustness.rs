//! Damaged and random files must never crash or hang the readers: each one
//! returns a capture or an error, quickly.
//!
//! The inputs are the built-in examples (every format), cut at many lengths,
//! with random bytes changed, plus random bytes and a few hand-made damaged
//! headers. The random numbers come from a fixed seed, so a failure can be
//! reproduced. Each case runs on its own thread with a time limit, so an
//! endless loop fails the test instead of stopping the run.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use waveglance::{examples, formats};

/// A small, deterministic random generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Open `bytes` (format detection, then reading) and fail the test if a
/// reader panics or does not finish within `limit`.
///
/// The reader runs on its own thread, so a reader that never ends (an
/// endless loop) also fails the test: the test stops waiting after
/// `limit`, and the test program ends with the thread still running.
fn must_not_crash(what: &str, bytes: &[u8], limit: Duration) {
    let bytes = bytes.to_vec();
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let _ = formats::detect(&bytes);
        let _ = formats::open(&bytes);
        let _ = done.send(());
    });
    match finished.recv_timeout(limit) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{what} did not finish in {limit:?}")
        }
        // The thread ended without sending: the reader panicked.
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("a reader panicked on {what}"),
    }
}

#[test]
fn cut_files_do_not_crash() {
    for e in examples::EXAMPLES {
        let n = e.bytes.len();
        // Many lengths near the start (headers), then spread over the file.
        let mut lengths: Vec<usize> = (0..n.min(600)).step_by(7).collect();
        lengths.extend((1..40).map(|k| n * k / 40));
        for len in lengths {
            must_not_crash(
                &format!("{} cut at {len}", e.file),
                &e.bytes[..len],
                Duration::from_secs(5),
            );
        }
    }
}

#[test]
fn changed_bytes_do_not_crash() {
    let mut rng = Rng(0x5EED_CAFE_F00D_1234);
    for e in examples::EXAMPLES {
        for round in 0..60 {
            let mut b = e.bytes.to_vec();
            // Change a few bytes; most rounds hit the header, where the
            // sizes and counts are.
            let header = b.len().min(1024);
            for _ in 0..1 + rng.below(8) {
                let at = if rng.below(4) == 0 {
                    rng.below(b.len())
                } else {
                    rng.below(header)
                };
                b[at] = match rng.below(4) {
                    0 => 0,
                    1 => 0xFF,
                    2 => b[at] ^ (1 << rng.below(8)),
                    _ => rng.next() as u8,
                };
            }
            must_not_crash(
                &format!("{} round {round}", e.file),
                &b,
                Duration::from_secs(5),
            );
        }
    }
}

#[test]
fn random_bytes_do_not_crash() {
    let mut rng = Rng(42);
    // Random data behind each format's signature, so the reader is chosen.
    let prefixes: [&[u8]; 8] = [
        b"",
        b"AG10",
        b"AG01",
        b"RG03",
        b"$date x $end $timescale 1 ns $end $var wire 1 ! a $end $enddefinitions $end",
        b"Time,CH1\n",
        b"\x00\x00\x00\x32",
        b"\xa5\xa5\xa4\x01",
    ];
    for prefix in prefixes {
        for len in [0usize, 1, 7, 64, 300, 2000, 15000, 70000] {
            let mut b = prefix.to_vec();
            b.extend((0..len).map(|_| rng.next() as u8));
            must_not_crash(
                &format!("{} random bytes after {prefix:?}", len),
                &b,
                Duration::from_secs(5),
            );
        }
    }
}

#[test]
fn absurd_counts_are_refused_quickly() {
    // A Keysight file that says it has u32::MAX waveforms, each with a
    // header size of 0.
    let mut b = b"AG10".to_vec();
    b.extend(1000u32.to_le_bytes());
    b.extend(u32::MAX.to_le_bytes());
    b.extend([0u8; 400]);
    must_not_crash(
        "Keysight with 4 billion waveforms",
        &b,
        Duration::from_secs(1),
    );
    assert!(formats::open(&b).is_err());
    // A VCD with very many signals.
    let mut v = String::from("$timescale 1 ns $end\n");
    for i in 0..5000 {
        v.push_str(&format!("$var wire 1 s{i} sig{i} $end\n"));
    }
    v.push_str("$enddefinitions $end\n#0\n1s0\n");
    must_not_crash(
        "VCD with 5000 signals",
        v.as_bytes(),
        Duration::from_secs(5),
    );
    // A VCD with 300 real variables (more than the 255 analog numbers).
    let mut v = String::from("$timescale 1 ns $end\n");
    for i in 0..300 {
        v.push_str(&format!("$var real 64 r{i} v{i} $end\n"));
    }
    v.push_str("$enddefinitions $end\n#0\n");
    for i in 0..300 {
        v.push_str(&format!("r1.5 r{i}\n"));
    }
    v.push_str("#10\n");
    must_not_crash(
        "VCD with 300 real variables",
        v.as_bytes(),
        Duration::from_secs(5),
    );
}

#!/usr/bin/env python3
"""Copy the waveform files from a FNIRSI DPOX180H connected by USB.

The USB disk of the DPOX180H can disconnect when it gets large, fast reads:
`cp` and the Finder fail on files of about 100 KB or more with "Device not
configured", and the scope must then be switched off and on. This script
reads in small pieces (4 KB), without the read-ahead of macOS, with a short
pause between pieces.

    python3 scripts/copy-dpox.py ~/Downloads/dpox

The script waits for the disk (up to 20 minutes), copies the smallest
files first, and keeps a file only when it is complete. If the disk
disconnects, the copied files stay; connect the scope again and the script
continues with the missing files. A file that is already in the destination
folder is not copied again.

Options:
    --source DIR     folder on the scope (default: /Volumes/NO NAME/Waveform)
    --wait MINUTES   how long to wait for the disk (default: 20)

Tip: connect the scope directly to the computer, not through a dock or hub,
and do not open the disk in the Finder while the copy runs.
"""

import argparse
import os
import sys
import time

CHUNK = 4096
PAUSE = 0.005  # seconds between pieces
F_NOCACHE = 48  # macOS fcntl: no buffer cache, so no read-ahead


def log(msg):
    print(time.strftime("%H:%M:%S"), msg, flush=True)


def no_cache(fd):
    """Turn off the read-ahead on macOS. Other systems: do nothing."""
    if sys.platform != "darwin":
        return
    import fcntl

    fcntl.fcntl(fd, F_NOCACHE, 1)


def copy(src, dst_dir, name):
    path = os.path.join(src, name)
    size = os.stat(path).st_size
    part = os.path.join(dst_dir, name + ".part")
    fd = os.open(path, os.O_RDONLY)
    done = 0
    try:
        no_cache(fd)
        with open(part, "wb") as out:
            while done < size:
                data = os.read(fd, CHUNK)
                if not data:
                    break
                out.write(data)
                done += len(data)
                time.sleep(PAUSE)
    finally:
        os.close(fd)
    if done != size:
        raise OSError(f"only {done} of {size} bytes")
    os.replace(part, os.path.join(dst_dir, name))
    log(f"copied {name} ({size} bytes)")


def files_on_scope(src):
    names = [
        n
        for n in os.listdir(src)
        if not n.startswith(".") and os.path.isfile(os.path.join(src, n))
    ]
    return sorted(names, key=lambda n: (os.stat(os.path.join(src, n)).st_size, n))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("destination", help="local folder for the copies")
    ap.add_argument("--source", default="/Volumes/NO NAME/Waveform")
    ap.add_argument("--wait", type=float, default=20, help="minutes")
    args = ap.parse_args()

    os.makedirs(args.destination, exist_ok=True)
    deadline = time.time() + args.wait * 60
    log(f"waiting for {args.source}")
    while time.time() < deadline:
        if not os.path.isdir(args.source):
            time.sleep(1)
            continue
        time.sleep(3)  # let the mount settle
        try:
            names = files_on_scope(args.source)
        except OSError as e:
            log(f"cannot list the folder: {e}")
            time.sleep(1)
            continue
        missing = [n for n in names if not os.path.exists(os.path.join(args.destination, n))]
        if not missing:
            log(f"done: {len(names)} files in {args.destination}")
            return 0
        log(f"{len(missing)} file(s) to copy")
        for name in missing:
            try:
                copy(args.source, args.destination, name)
            except OSError as e:
                log(f"failed on {name}: {e}; connect the scope again")
                break
    log("stopped: the disk did not come back in time")
    return 1


if __name__ == "__main__":
    sys.exit(main())

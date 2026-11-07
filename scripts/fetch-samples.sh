#!/usr/bin/env bash
# Download real DPOX180H .wav files for the integration tests.
#
# The files come from github.com/Rav263/fnirsi-wave-decoder. That repository
# has no license, so the files are NOT committed here: this script copies
# them into tests/data/ (git-ignored).
set -euo pipefail

REPO="https://github.com/Rav263/fnirsi-wave-decoder.git"
COMMIT="66d45400eb5d8ab18f54e8c2ab39e6fb23c2b92b"   # pinned: tests expect these files

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/tests/data/dpox180h"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git -C "$TMP" init -q
git -C "$TMP" remote add origin "$REPO"
git -C "$TMP" fetch -q --depth 1 origin "$COMMIT"
git -C "$TMP" checkout -q FETCH_HEAD -- dpox180h_tests

mkdir -p "$DEST/Waveform-google"
cp "$TMP"/dpox180h_tests/*.wav "$DEST/"
cp "$TMP"/dpox180h_tests/Waveform-google/*.wav "$DEST/Waveform-google/"
echo "Copied $(find "$DEST" -name '*.wav' | wc -l | tr -d ' ') files to $DEST"

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

# FNIRSI 1013D files (and screenshots) from github.com/lexus2026HA/FNIRSI-2C53T-Decoder.
# That repository is CC BY-NC 4.0, so the files are NOT committed here either.
REPO2="https://github.com/lexus2026HA/FNIRSI-2C53T-Decoder.git"
COMMIT2="5bdd72d14af1913d2cec81adfe5b09e1a45ac0fa"   # pinned: tests expect these files
DEST2="$ROOT/tests/data/fnirsi1013d"
TMP2="$(mktemp -d)"
trap 'rm -rf "$TMP" "$TMP2"' EXIT
git -C "$TMP2" init -q
git -C "$TMP2" remote add origin "$REPO2"
git -C "$TMP2" fetch -q --depth 1 origin "$COMMIT2"
git -C "$TMP2" checkout -q FETCH_HEAD -- samples/1013d
mkdir -p "$DEST2"
cp "$TMP2"/samples/1013d/* "$DEST2/"
echo "Copied $(find "$DEST2" -name '*.wav' | wc -l | tr -d ' ') files to $DEST2"

# Keysight/Agilent .bin files (with the scope settings as .txt and a
# screenshot as .png) from github.com/scottprahl/RigolWFM (BSD-3-Clause).
# Not committed either; the tests only need them locally.
REPO3="https://github.com/scottprahl/RigolWFM.git"
COMMIT3="9e6f993103f773f81df409fc65c2cc359d7bd249"   # pinned: tests expect these files
DEST3="$ROOT/tests/data/keysight"
TMP3="$(mktemp -d)"
trap 'rm -rf "$TMP" "$TMP2" "$TMP3"' EXIT
git -C "$TMP3" init -q
git -C "$TMP3" remote add origin "$REPO3"
git -C "$TMP3" fetch -q --depth 1 origin "$COMMIT3"
git -C "$TMP3" checkout -q FETCH_HEAD -- tests/files/bin tests/files/wfm
mkdir -p "$DEST3"
cp "$TMP3"/tests/files/bin/agilent_* "$DEST3/"
echo "Copied $(find "$DEST3" -name '*.bin' | wc -l | tr -d ' ') files to $DEST3"

# Rigol .bin and .wfm files, with the CSV files that the scopes exported for
# some of them (same repository and commit).
DEST4="$ROOT/tests/data/rigol"
mkdir -p "$DEST4"
cp "$TMP3"/tests/files/bin/MSO50* "$TMP3"/tests/files/bin/DHO* "$DEST4/"
cp "$TMP3"/tests/files/wfm/DS1054Z-* "$TMP3"/tests/files/wfm/DS1074Z-* "$TMP3"/tests/files/wfm/DS1202Z-E.wfm \
   "$TMP3"/tests/files/wfm/MSO1104.wfm "$TMP3"/tests/files/wfm/DZ1104Z-A.wfm \
   "$TMP3"/tests/files/wfm/DS1000E-* "$TMP3"/tests/files/wfm/DS1052E.* "$TMP3"/tests/files/wfm/DS1102E-* \
   "$TMP3"/tests/files/wfm/DS1102D-* "$TMP3"/tests/files/wfm/DS2072A-* "$TMP3"/tests/files/wfm/DS2000-* \
   "$TMP3"/tests/files/wfm/DS2202.wfm "$DEST4/"
echo "Copied $(find "$DEST4" -name '*.wfm' -o -name '*.bin' | wc -l | tr -d ' ') files to $DEST4"

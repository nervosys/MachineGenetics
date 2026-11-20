#!/usr/bin/env python3
"""Standard-compressor reference points for the arena's bits-per-byte figures.

Usage:
    python3 scripts/compression-reference.py FILE [FILE...] [--json OUT]

For each file, prints its size, SHA-256 and bits per byte under:

  order-0   ideal order-0 coder: empirical byte-frequency entropy, no table cost
  zlib      zlib.compress(level 9)
  bz2       bz2.compress(level 9)
  xz        lzma, FORMAT_XZ, preset 9 | PRESET_EXTREME

bits per byte = 8 * compressed_bytes / original_bytes. The compressor figures
include container and header overhead (zlib 6 bytes; bz2 and xz tens of
bytes), so they overstate the cost on small files. A "concatenated" row per
compressor compresses all the files, in argument order, as one input.

Standard library only. Output is deterministic: no timestamps, paths are
echoed as given, and the JSON uses sorted keys.
"""

import argparse
import bz2
import hashlib
import json
import lzma
import math
import sys
import zlib
from collections import Counter

COMPRESSORS = (
    ("zlib", lambda d: zlib.compress(d, 9)),
    ("bz2", lambda d: bz2.compress(d, 9)),
    ("xz", lambda d: lzma.compress(
        d, format=lzma.FORMAT_XZ, preset=9 | lzma.PRESET_EXTREME)),
)


def order0_bits_per_byte(data):
    """Ideal order-0 coder: sum of n_c * log2(n / n_c) over byte values, / n.

    Symbols are visited in sorted order so the float sum is reproducible.
    """
    n = len(data)
    if n == 0:
        return None
    counts = Counter(data)
    total = 0.0
    for symbol in sorted(counts):
        c = counts[symbol]
        total += c * math.log2(n / c)
    return total / n


def compressed_bpb(data, compress):
    if not data:
        return None
    return 8.0 * len(compress(data)) / len(data)


def measure(data):
    row = {"bytes": len(data), "order0": order0_bits_per_byte(data)}
    for name, compress in COMPRESSORS:
        row[name] = compressed_bpb(data, compress)
    return row


def fmt(value):
    return "n/a" if value is None else "%.3f" % value


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Standard-compressor bits-per-byte reference for FILE...")
    parser.add_argument("files", nargs="+", metavar="FILE")
    parser.add_argument("--json", metavar="OUT",
                        help="also write the figures as JSON to OUT")
    args = parser.parse_args(argv)

    files = []
    blobs = []
    for path in args.files:
        with open(path, "rb") as handle:
            data = handle.read()
        blobs.append(data)
        files.append({
            "path": path,
            "sha256": hashlib.sha256(data).hexdigest(),
            **measure(data),
        })

    concatenated = b"".join(blobs)
    total = measure(concatenated)

    print("compression-reference: bits per byte = 8 * compressed / original.")
    print("Container and header overhead is included, which inflates the")
    print("compressor figures on small files. order-0 is the ideal order-0")
    print("coder (byte-frequency entropy, no table cost). Each file is")
    print("compressed alone; the concatenated row is all files in argument order.")
    print()
    header = "%-40s %9s  %-64s %8s %8s %8s %8s" % (
        "file", "bytes", "sha256", "order-0", "zlib-9", "bz2-9", "xz-9e")
    print(header)
    for f in files:
        print("%-40s %9d  %-64s %8s %8s %8s %8s" % (
            f["path"], f["bytes"], f["sha256"], fmt(f["order0"]),
            fmt(f["zlib"]), fmt(f["bz2"]), fmt(f["xz"])))
    print("%-40s %9d  %-64s %8s %8s %8s %8s" % (
        "concatenated", total["bytes"], hashlib.sha256(concatenated).hexdigest(),
        fmt(total["order0"]), fmt(total["zlib"]), fmt(total["bz2"]),
        fmt(total["xz"])))

    if args.json:
        report = {
            "method": "bits per byte = 8 * compressed_bytes / original_bytes; "
                      "includes container overhead",
            "compressors": ["order0", "zlib", "bz2", "xz"],
            "files": files,
            "concatenated": {"paths": args.files, **total},
        }
        with open(args.json, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(report, handle, indent=2, sort_keys=True)
            handle.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

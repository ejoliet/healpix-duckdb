#!/usr/bin/env python3
"""Throughput benchmark for hpx_ang2pix over a 10M-row table.

Sealed benchmark for the self-improvement loop (see
.omc/self-improve/topics/default/config/goal.md). Builds the release
extension, loads it into the pinned DuckDB CLI, and times
`sum(hpx_ang2pix(29, ra, dec))` over a deterministic synthetic table.

Run with: uv run tools/bench/ang2pix_bench.py

Prints one JSON object as the last stdout line:
    {"primary": <rows_per_sec>, "sub_scores": {...}}
Exits non-zero on build failure, query failure, or checksum mismatch.
"""

import json
import re
import subprocess
import sys
from pathlib import Path
from statistics import median

REPO = Path(__file__).resolve().parents[2]
EXTENSION = REPO / "build" / "release" / "extension" / "healpix" / "healpix.duckdb_extension"
DUCKDB = "duckdb"

ROWS = 10_000_000
TIMED_RUNS = 15

# AIDEV-NOTE: the arithmetic below is the whole fixture. It must stay
# bit-identical across runs, so it uses DuckDB's `hash()` over `range(n)`
# rather than random(): same rows every run, no seed state, no I/O.
# ra lands in [0, 360) and dec in [-90, 90], the exact domain
# check_ra_dec() accepts, so no row is rejected and the poles are included.
GEN = (
    "SELECT (hash(i) % 36000000) / 100000.0 AS ra, "
    "(hash(i + 1000000007) % 18000001) / 100000.0 - 90.0 AS dec "
    f"FROM range({ROWS}) s(i)"
)

# Guard against a "faster" implementation that returns different pixels.
# sum() over BIGINT widens to HUGEINT in DuckDB, so this is exact.
EXPECTED_CHECKSUM = 17296449203886822776082728

QUERY = "SELECT sum(hpx_ang2pix(29, ra, dec)) FROM t;"


def fail(msg: str) -> None:
    print(f"ang2pix_bench: {msg}", file=sys.stderr)
    sys.exit(1)


def build() -> None:
    r = subprocess.run(["make", "release"], cwd=REPO, capture_output=True, text=True)
    if r.returncode != 0:
        fail(f"make release failed:\n{r.stdout[-4000:]}\n{r.stderr[-4000:]}")
    if not EXTENSION.is_file():
        fail(f"release extension missing at {EXTENSION}")


def run_duckdb() -> str:
    script = "\n".join(
        [
            f"LOAD '{EXTENSION}';",
            ".mode list",
            ".header off",
            "SELECT 'THREADS=' || current_setting('threads');",
            f"CREATE TABLE t AS {GEN};",
            # Warm-up: pages touched, code paths faulted in, not timed.
            QUERY,
            ".timer on",
        ]
        + [QUERY] * TIMED_RUNS
    )
    r = subprocess.run(
        [DUCKDB, "-unsigned"], input=script, cwd=REPO, capture_output=True, text=True
    )
    if r.returncode != 0 or "Error" in r.stderr:
        fail(f"duckdb failed (rc={r.returncode}):\n{r.stdout[-4000:]}\n{r.stderr[-4000:]}")
    return r.stdout


def main() -> None:
    build()
    out = run_duckdb()

    threads = next((l.split("=", 1)[1] for l in out.splitlines() if l.startswith("THREADS=")), None)
    if threads is None:
        fail(f"no THREADS line in output:\n{out[-2000:]}")

    sums = [int(l) for l in out.splitlines() if re.fullmatch(r"-?\d+", l.strip())]
    # 1 warm-up + TIMED_RUNS checksums; every one must match.
    if len(sums) != TIMED_RUNS + 1 or any(s != EXPECTED_CHECKSUM for s in sums):
        fail(
            f"checksum guard failed: expected {TIMED_RUNS + 1} rows of "
            f"{EXPECTED_CHECKSUM}, got {sums}"
        )

    runs = [float(m) for m in re.findall(r"Run Time \(s\): real ([0-9.]+)", out)]
    if len(runs) != TIMED_RUNS:
        fail(f"expected {TIMED_RUNS} timings, got {runs}")

    med = median(runs)
    if med <= 0:
        fail(f"non-positive median time: {runs}")

    print(
        json.dumps(
            {
                "primary": ROWS / med,
                "sub_scores": {
                    "median_s": med,
                    "runs_s": runs,
                    "threads": int(threads),
                    "checksum_ok": True,
                },
            }
        )
    )


if __name__ == "__main__":
    main()

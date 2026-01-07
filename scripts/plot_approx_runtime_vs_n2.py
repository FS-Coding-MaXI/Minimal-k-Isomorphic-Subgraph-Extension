#!/usr/bin/env python3
"""
Add a Python script to plot approx runtime vs |VH| for fixed k and |VG|

Figure 2: Execution time of the Approximation Algorithm vs. |VH| for fixed k and |VG|.

This script:
- Ensures Rust binaries (approx_solver, input-generator) are built
- Generates problem instances with fixed |VG| = n1 and varying |VH| = n2
- Benchmarks execution time of the approximation solver for each instance
- Produces a plot of runtime vs |VH| (n2) for fixed |VG| (n1) and k

Usage (from project root):
  python scripts/plot_approx_runtime_vs_n2.py \
    --n1 8 \
    --k 3 \
    --n2-start 10 \
    --n2-end 50 \
    --n2-step 5 \
    --density-g 0.35 \
    --density-h 0.20 \
    --trials-multiplier 10 \
    --timeout-seconds 60 \
    --output ./approx_runtime_vs_n2_n1-8_k-3.png

Notes:
- Instances are written under examples/generated_benchmarks/.
- The plot uses a log scale on the y-axis to capture orders of magnitude if needed.
"""

import argparse
import subprocess
import sys
import time
from pathlib import Path
from typing import List, Optional, Tuple

import matplotlib.pyplot as plt


PROJECT_ROOT = Path(__file__).resolve().parents[1]
BIN_DIR = PROJECT_ROOT / "target" / "release"
EXAMPLES_DIR = PROJECT_ROOT / "examples"
GEN_DIR = EXAMPLES_DIR / "generated_benchmarks"


def run_cmd(cmd: List[str], cwd: Path, timeout: Optional[int] = None) -> Tuple[int, str]:
    """
    Run command and return (exit_code, combined_output).
    """
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(cwd),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
            text=True,
            check=False,
        )
        return proc.returncode, proc.stdout
    except subprocess.TimeoutExpired as e:
        return 124, f"TimeoutExpired: {e}"


def ensure_built() -> None:
    """
    Build required binaries if not present.
    """
    need_build = False
    for name in ("approx_solver", "input-generator"):
        if not (BIN_DIR / name).exists():
            need_build = True
            break

    if need_build:
        print("Building release binaries with cargo build --release ...")
        code, out = run_cmd(["cargo", "build", "--release"], PROJECT_ROOT)
        print(out)
        if code != 0:
            print("Failed to build project. Please check the output above.")
            sys.exit(1)


def generate_instance(n1: int, n2: int, density_g: float, density_h: float, seed: Optional[int], output_path: Path) -> None:
    """
    Generate an instance file using input-generator.
    """
    output_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(BIN_DIR / "input-generator"),
        "--n1", str(n1),
        "--n2", str(n2),
        "--density-g", str(density_g),
        "--density-h", str(density_h),
        "--noise",
        "--output", str(output_path),
    ]
    if seed is not None:
        cmd.extend(["--seed", str(seed)])

    code, out = run_cmd(cmd, PROJECT_ROOT)
    print(f"[gen n1={n1}, n2={n2}] exit={code}")
    if code != 0:
        print(out)
        raise RuntimeError(f"Failed to generate instance: {output_path}")
    else:
        lines = out.strip().splitlines()
        print("\n".join(lines[:10]))


def time_solver_approx(instance_path: Path, k: int, trials_multiplier: int, timeout_seconds: int) -> Optional[float]:
    """
    Time the approximation solver. Returns seconds or None if failure/timeout.
    """
    cmd = [
        str(BIN_DIR / "approx_solver"),
        "--input", str(instance_path),
        "--k", str(k),
        "--trials-multiplier", str(trials_multiplier),
    ]
    start = time.perf_counter()
    code, out = run_cmd(cmd, PROJECT_ROOT, timeout=timeout_seconds)
    elapsed = time.perf_counter() - start
    print(f"[approx] n2={instance_path.stem.split('_')[2]}, exit={code}, time={elapsed:.6f}s")
    if code != 0:
        print(out)
        return None
    return elapsed


def plot_runtime(n2_values: List[int], approx_times: List[Optional[float]], n1: int, k: int, output_path: Path, log_scale: bool = True) -> None:
    """
    Produce the figure and save to output_path.
    """
    plt.figure(figsize=(8, 5))

    xs, ys = [], []
    for x, y in zip(n2_values, approx_times):
        if y is not None and y > 0:
            xs.append(x)
            ys.append(y)

    if xs:
        plt.plot(xs, ys, marker="o", linestyle="-", color="tab:blue", label="Approximation")

    if log_scale:
        plt.yscale("log")

    plt.xlabel("|VH| (n2)")
    plt.ylabel("Execution time (seconds{} )".format(", log scale" if log_scale else ""))
    plt.title(f"Approximation Runtime vs |VH| (fixed |VG|={n1}, k={k})")
    plt.grid(True, which="both", linestyle="--", alpha=0.4)
    plt.legend()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    plt.tight_layout()
    plt.savefig(output_path, dpi=200)
    print(f"Saved figure to: {output_path}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Benchmark Approximation runtime vs |VH| for fixed |VG| and k.")
    parser.add_argument("--n1", type=int, required=True, help="Fixed |VG| size (pattern graph vertices).")
    parser.add_argument("--k", type=int, required=True, help="Number of distinct isomorphic mappings required.")
    parser.add_argument("--n2-start", type=int, required=True, help="Starting |VH| size.")
    parser.add_argument("--n2-end", type=int, required=True, help="Ending |VH| size (inclusive).")
    parser.add_argument("--n2-step", type=int, default=1, help="Step for |VH|.")
    parser.add_argument("--density-g", type=float, default=0.35, help="Edge probability for G.")
    parser.add_argument("--density-h", type=float, default=0.20, help="Edge probability for H.")
    parser.add_argument("--seed", type=int, default=12345, help="RNG seed for reproducibility.")
    parser.add_argument("--trials-multiplier", type=int, default=10, help="Approx solver trials multiplier.")
    parser.add_argument("--timeout-seconds", type=int, default=60, help="Per-run timeout for approx solver.")
    parser.add_argument("--output", type=str, default=str(PROJECT_ROOT / "approx_runtime_vs_n2.png"), help="Path to save the figure.")
    parser.add_argument("--no-log-scale", action="store_true", help="Disable log scale on y-axis.")
    return parser.parse_args()


def main():
    args = parse_args()
    n1 = args.n1
    k = args.k
    n2_values = list(range(args.n2_start, args.n2_end + 1, args.n2_step))
    log_scale = not args.no_log_scale

    print(f"Project root: {PROJECT_ROOT}")
    print(f"Fixed n1 (|VG|): {n1}, varying n2 (|VH|): {n2_values}, k: {k}")
    ensure_built()

    approx_times: List[Optional[float]] = []

    for n2 in n2_values:
        instance_path = GEN_DIR / f"generated_{n1}_{n2}.txt"
        if not instance_path.exists():
            print(f"Generating instance for n1={n1}, n2={n2} ...")
            generate_instance(n1, n2, args.density_g, args.density_h, args.seed, instance_path)
        else:
            print(f"Using existing instance: {instance_path}")

        # Benchmark approx
        t_approx = time_solver_approx(instance_path, k, args.trials_multiplier, args.timeout_seconds)
        approx_times.append(t_approx)

    # Summaries
    print("\nSummary (seconds):")
    for n2, ta in zip(n2_values, approx_times):
        ta_str = f"{ta:.6f}" if ta is not None else "fail/timeout"
        print(f"n2={n2}, approx={ta_str}")

    # Plot
    output_path = Path(args.output).resolve()
    plot_runtime(n2_values, approx_times, n1, k, output_path, log_scale=log_scale)


if __name__ == "__main__":
    main()

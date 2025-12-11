#!/usr/bin/env python3
"""
Generate "Figure 1: Log-scale comparison of execution time between Exact and Approximation algorithms
as a function of graph size (|VH| fixed, varying |VG|)."

This script:
- Builds the Rust project binaries (exact-solver, approx-solver, input-generator)
- Generates problem instances with fixed |VH| = n2 and varying |VG| = n1
- Benchmarks execution time of both solvers for each instance
- Produces a log-scale plot comparing runtime vs |VG| (n1) for a fixed |VH| (n2)

Usage:
  python scripts/plot_runtime_vs_n2.py \
    --n2 25 \
    --k 2 \
    --n1-start 2 \
    --n1-end 12 \
    --n1-step 1 \
    --density-g 0.35 \
    --density-h 0.20 \
    --trials-multiplier 10 \
    --timeout-seconds 30 \
    --output ./runtime_vs_n1_n2-25_k-2.png

Notes:
- Exact solver may be infeasible for larger n1; we skip results on timeout or non-zero exit.
- Approximation solver supports --trials-multiplier to trade time for solution quality.
- All generated instances are written under examples/generated_benchmarks/.
- Y-axis is log scale to show differences over orders of magnitude.
"""

import argparse
import subprocess
import sys
import time
from pathlib import Path
from typing import List, Optional, Tuple

from matplotlib import pyplot as plt


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
    for name in ("exact-solver", "approx-solver", "input-generator"):
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
    print(cmd)

    code, out = run_cmd(cmd, PROJECT_ROOT)
    print(f"[gen n1={n1}, n2={n2}] exit={code}")
    if code != 0:
        print(out)
        raise RuntimeError(f"Failed to generate instance: {output_path}")
    else:
        # Print only first few lines of generator stats to keep logs short
        lines = out.strip().splitlines()
        print("\n".join(lines[:10]))


def time_solver_exact(instance_path: Path, k: int, timeout_seconds: int) -> Optional[float]:
    """
    Time the exact solver. Returns seconds or None if infeasible/timeout/failure.
    """
    cmd = [
        str(BIN_DIR / "exact-solver"),
        "--input", str(instance_path),
        "--k", str(k),
    ]
    start = time.perf_counter()
    code, out = run_cmd(cmd, PROJECT_ROOT, timeout=timeout_seconds)
    elapsed = time.perf_counter() - start
    print(f"[exact] n1={instance_path.stem.split('_')[1]}, exit={code}, time={elapsed:.6f}s")
    if code != 0:
        print(out)
        return None
    return elapsed


def time_solver_approx(instance_path: Path, k: int, trials_multiplier: int, timeout_seconds: int) -> Optional[float]:
    """
    Time the approximation solver. Returns seconds or None if failure/timeout.
    """
    cmd = [
        str(BIN_DIR / "approx-solver"),
        "--input", str(instance_path),
        "--k", str(k),
        "--trials-multiplier", str(trials_multiplier),
    ]
    start = time.perf_counter()
    code, out = run_cmd(cmd, PROJECT_ROOT, timeout=timeout_seconds)
    elapsed = time.perf_counter() - start
    print(f"[approx] n1={instance_path.stem.split('_')[1]}, exit={code}, time={elapsed:.6f}s")
    if code != 0:
        print(out)
        return None
    return elapsed


def plot_runtime(n1_values: List[int], exact_times: List[Optional[float]], approx_times: List[Optional[float]], n2: int, k: int, output_path: Path) -> None:
    """
    Produce the figure with log-scale y and save to output_path.
    """
    plt.figure(figsize=(8, 5))
    # Prepare plotting with masking None values
    def mask_vals(xs, ys):
        mx, my = [], []
        for x, y in zip(xs, ys):
            if y is not None and y > 0:
                mx.append(x)
                my.append(y)
        return mx, my

    x_exact, y_exact = mask_vals(n1_values, exact_times)
    x_approx, y_approx = mask_vals(n1_values, approx_times)

    if x_exact:
        plt.plot(x_exact, y_exact, marker="o", linestyle="-", color="tab:red", label="Exact")
    if x_approx:
        plt.plot(x_approx, y_approx, marker="s", linestyle="-", color="tab:blue", label="Approximation")

    plt.yscale("log")
    plt.xlabel("|VG| (n1)")
    plt.ylabel("Execution time (seconds)")
    plt.title(f"Execution Time vs |VG| (|VH|={n2} fixed, k={k})")
    plt.grid(True, which="both", linestyle="--", alpha=0.4)
    plt.legend()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    plt.tight_layout()
    plt.savefig(output_path, dpi=200)
    print(f"Saved figure to: {output_path}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Benchmark Exact vs Approximation runtime vs |VG| for fixed |VH|.")
    parser.add_argument("--n2", type=int, required=True, help="Fixed |VH| size (host graph vertices).")
    parser.add_argument("--k", type=int, required=True, help="Number of distinct isomorphic mappings required.")
    parser.add_argument("--n1-start", type=int, default=2, help="Starting |VG| size.")
    parser.add_argument("--n1-end", type=int, default=9, help="Ending |VG| size (inclusive).")
    parser.add_argument("--n1-step", type=int, default=1, help="Step for |VG|.")
    parser.add_argument("--density-g", type=float, default=0.35, help="Edge probability for G.")
    parser.add_argument("--density-h", type=float, default=0.20, help="Edge probability for H.")
    parser.add_argument("--seed", type=int, default=12345, help="RNG seed for reproducibility.")
    parser.add_argument("--trials-multiplier", type=int, default=10, help="Approx solver trials multiplier.")
    parser.add_argument("--timeout-seconds", type=int, default=60, help="Per-run timeout for solvers.")
    parser.add_argument("--output", type=str, default=str(PROJECT_ROOT / "runtime_vs_n1.png"), help="Path to save the figure.")
    return parser.parse_args()


def main():
    args = parse_args()
    n2 = args.n2
    k = args.k
    n1_values = list(range(args.n1_start, args.n1_end + 1, args.n1_step))

    print(f"Project root: {PROJECT_ROOT}")
    print(f"Using fixed n2 (|VH|): {n2}, varying n1 (|VG|): {n1_values}, k: {k}")
    ensure_built()

    exact_times: List[Optional[float]] = []
    approx_times: List[Optional[float]] = []
    exact_failed = False

    for n1 in n1_values:
        instance_path = GEN_DIR / f"generated_{n1}_{n2}.txt"
        if not instance_path.exists():
            print(f"Generating instance for n1={n1}, n2={n2} ...")
            generate_instance(n1, n2, args.density_g, args.density_h, args.seed, instance_path)
        else:
            print(f"Using existing instance: {instance_path}")

        # Benchmark exact (with early stop on first failure/timeout)
        if not exact_failed:
            t_exact = time_solver_exact(instance_path, k, args.timeout_seconds)
            if t_exact is None:
                exact_failed = True
            exact_times.append(t_exact)
        else:
            print(f"[exact] skipping for n1={n1} after previous failure/timeout")
            exact_times.append(None)

        # Benchmark approx
        t_approx = time_solver_approx(instance_path, k, args.trials_multiplier, args.timeout_seconds)
        approx_times.append(t_approx)

    # Summaries
    print("\nSummary (seconds):")
    for n1, te, ta in zip(n1_values, exact_times, approx_times):
        te_str = f"{te:.6f}" if te is not None else "infeasible/timeout/fail"
        ta_str = f"{ta:.6f}" if ta is not None else "fail"
        print(f"n1={n1}, exact={te_str}, approx={ta_str}")

    # Plot
    output_path = Path(args.output).resolve()
    plot_runtime(n1_values, exact_times, approx_times, n2, k, output_path)


if __name__ == "__main__":
    main()

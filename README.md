# Minimal k-Isomorphic Subgraph Extension

A Rust implementation for solving the Minimal k-Isomorphic Subgraph Extension problem.

## Problem Statement

### Minimal k-Isomorphic Subgraph Extension

We operate on **directed multigraphs**, where the adjacency matrix **A ∈ ℕⁿˣⁿ** stores the count of edges A[i][j] from vertex i to vertex j.

An **isomorphism** from G to a subgraph of H is defined by an **injective mapping** f: V₁ → V₂.

The **cost** of implementing a single mapping f onto H is the total number of edges that must be added to H to satisfy the structure of G. For multigraphs, this cost is:

**C(f) = Σᵤ∈V₁ Σᵥ∈V₁ max(0, Aɢ[u][v] - Aₕ[f(u)][f(v)])**

We need to find a minimal extension H′ such that it includes a **set of k distinct mappings**, S = {f₁, f₂, ..., fₖ}.

### Exact Algorithm

The exact algorithm identifies every possible way to map G onto H and then checks every possible combination of k such mappings to find the one with the globally minimal combined cost.

**Steps:**
1. **Enumerate All Mappings:** Generate the set ℱ of *all possible* injective mappings from V₁ to V₂. This is done using a backtracking (DFS) search.
2. **Enumerate k-Subsets:** Generate all possible subsets S ⊂ ℱ such that |S| = k.
3. **Calculate Combined Cost:** For each subset S, calculate its total combined cost C(S).
4. **Find Minimum:** The solution is the minimum C(S) found among all k-subsets.

This process guarantees finding the optimal solution because it exhaustively checks every single valid combination.

## Project Structure

This project provides three solver binaries:

- **Interactive TUI Solver** (`solver`): Terminal User Interface with real-time visualization
  - Supports both exact and approximation algorithms
  - Shows live progress, mapping visualization, and results
  - Interactive controls for algorithm selection and execution
  
- **Exact Solution** (`exact_solver`): Command-line solver for optimal minimal extension using exhaustive search
  - Complexity: O(k·n₁²·n₂^(k·n₁))
  - Uses parallel processing with Rayon for better performance
  - Guarantees optimal solution
  
- **Approximation Solution** (`approx_solver`): Command-line solver with polynomial time complexity
  - Complexity: O(k·n₁³·n₂²)
  - Uses Sequential Greedy Extension algorithm with randomized trials
  - Much faster on large inputs, but may not find optimal solution

## Building

```bash
# Build all binaries (interactive TUI solver, exact solver, approx solver, input generator)
cargo build --release

# Build only interactive TUI solver
cargo build --release --bin solver

# Build only exact solver
cargo build --release --bin exact_solver

# Build only approximation solver
cargo build --release --bin approx_solver

# Build only input generator
cargo build --release --bin input-generator
```

## Running

### Interactive TUI Solver
```bash
# Using cargo
cargo run --release --bin solver -- --input examples/example1_3x4.txt --k 2

# Using compiled binary
./target/release/solver --input examples/example1_3x4.txt --k 2

# Choose algorithm interactively
./target/release/solver --input examples/example1_3x4.txt --k 2 --algorithm exact
./target/release/solver --input examples/example1_3x4.txt --k 2 --algorithm approx
```

The TUI solver provides:
- Real-time progress visualization
- Mapping table display with color-coded highlights
- Interactive navigation (arrow keys, page up/down)
- Detailed statistics and results
- Choice between exact and approximation algorithms

### Exact Solver
```bash
# Using cargo
cargo run --release --bin exact_solver -- --input examples/example1_3x4.txt --k 2

# Using compiled binary
./target/release/exact_solver --input examples/example1_3x4.txt --k 2
```

### Approximation Solver
```bash
# Using cargo
cargo run --release --bin approx_solver -- --input examples/example1_3x4.txt --k 2

# Using compiled binary
./target/release/approx_solver --input examples/example1_3x4.txt --k 2

# With increased trials for better quality (10x more trials)
./target/release/approx_solver --input examples/example1_3x4.txt --k 2 --trials-multiplier 10

# With significantly more trials (100x) for harder problems
./target/release/approx_solver --input examples/example6_10x25.txt --k 5 -t 100
```

### Input Generator

```bash
# Basic instance (writes to file)
cargo run --release --bin input-generator -- --n1 3 --n2 6 --output examples/generated_3_6.txt

# Instance with custom densities and seed
cargo run --release --bin input-generator -- --n1 5 --n2 9 --density-g 0.5 --density-h 0.15 --seed 12345 --output examples/generated_5_9.txt

# Instance disabling noise
cargo run --release --bin input-generator -- --n1 4 --n2 10 --noise false --output examples/generated_4_10_no_noise.txt
```

Generates a problem instance (two graphs G then H) with a partially satisfied hidden embedding; the raw file contains only the numeric format (no comments). Generation statistics are printed to stdout, while the instance itself is always written to the specified output path.

### Command Line Arguments

**Interactive TUI Solver:**
- `--input <path>` or `-i <path>`: Path to input file containing graph descriptions
- `--k <number>` or `-k <number>`: Number of distinct isomorphic mappings required
- `--algorithm <type>` or `-a <type>`: Algorithm to use (exact, approx, approximate, approximation)

**Exact Solver:**
- `--input <path>` or `-i <path>`: Path to input file containing graph descriptions
- `--k <number>` or `-k <number>`: Number of distinct isomorphic mappings required

**Approximation Solver:**
- `--input <path>` or `-i <path>`: Path to input file containing graph descriptions
- `--k <number>` or `-k <number>`: Number of distinct isomorphic mappings required
- `--trials-multiplier <number>` or `-t <number>`: Multiplier for number of randomized trials (default: 1)
  - Number of trials per mapping = n₁ × n₂ × multiplier
  - Higher values increase computation time but may improve solution quality
  - Recommended: 1 for quick results, 10-100 for better quality on hard instances

**Input Generator:**
- `--n1 <number>`: Vertex count of pattern graph G (must be > 0)
- `--n2 <number>`: Vertex count of host graph H (must be > n1)
- `--density-g <float>`: Edge probability for G (default: 0.35)
- `--density-h <float>`: Edge probability for base H (default: 0.20)
- `--multiedge-prob <float>`: Probability an existing edge becomes multiedge (default: 0.15)
- `--max-multiedge <number>`: Max multiplicity for multiedges (default: 4)
- `--embed-strength <float>`: Fraction of G edges forced satisfied in H under a hidden mapping (default: 0.40)
- `--deficit-strength <float>`: Fraction of G edges forced under-satisfied (needs extension) (default: 0.35)
- `--noise` / `--noise false`: Enable/disable light noise among unused H vertices (default: true)
- `--noise-max <number>`: Max multiplicity for noise edges (default: 1)
- `--seed <number>`: Fixed RNG seed for reproducibility (optional)
- `--output <path>`: Mandatory. Path to write instance file (pure format: no comments, only two matrices). Stats are printed to stdout.
- (comments and self-loops removed: generator never outputs headers, omits self-loops)

## Input Format

The input file should contain two graphs in the following format:

```
<n1>
<adjacency matrix for graph G (pattern) - n1 rows>
<n2>
<adjacency matrix for graph H (host) - n2 rows>
```

Each row of the adjacency matrix should contain space-separated integers representing edge counts between vertices (supports multigraphs).

See `examples/` directory for sample input files.

## Performance Comparison

| Example | Exact Solver | Approx Solver | Quality |
|---------|-------------|---------------|---------|
| Example 1 (2→3, k=2) | 2 edges, <0.001s | 2 edges, <0.001s | Optimal |
| Example 3 (3→4, k=2) | 1 edge, <0.001s | 1 edge, <0.001s | Optimal |
| Example 5 (5→8, k=2) | 11 edges, ~6s | 12 edges, ~0.001s | 12 edges  |
| Example 6 (10→25, k=1) | Infeasible | 16 edges, ~0.003s | Unknown |
| Example 6 (10→25, k=3) | Infeasible | 20 edges, ~0.008s | Unknown |
| Example 6 (10→25, k=5) | Infeasible | 28 edges, ~0.013s | Unknown |

The approximation solver is recommended for large problem instances where exact computation is infeasible.

**Note:** For the large example (10→25 vertices), the exact solver would need to evaluate approximately C(3.6×10¹³, k) combinations, making it computationally infeasible

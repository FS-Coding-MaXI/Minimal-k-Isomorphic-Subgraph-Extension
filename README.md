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

This project provides two solution approaches:

- **Exact Solution** (`exact-solver`): Computes the optimal minimal extension using exhaustive search
  - Complexity: O(k·n₁²·n₂^(k·n₁))
  - Uses parallel processing with Rayon for better performance
  - Guarantees optimal solution
  
- **Approximation Solution** (`approx-solver`): Provides an approximate solution with polynomial time complexity
  - Complexity: O(k·n₁³·n₂²)
  - Uses Sequential Greedy Extension algorithm with randomized trials
  - Much faster on large inputs, but may not find optimal solution

## Building

```bash
# Build both solvers
cargo build --release

# Build only exact solver
cargo build --release --bin exact-solver

# Build only approximation solver
cargo build --release --bin approx-solver
```

## Running

### Exact Solver
```bash
# Using cargo
cargo run --release --bin exact-solver -- --input examples/example1.txt --k 2

# Using compiled binary
./target/release/exact-solver --input examples/example1.txt --k 2
```

### Approximation Solver
```bash
# Using cargo
cargo run --release --bin approx-solver -- --input examples/example1.txt --k 2

# Using compiled binary
./target/release/approx-solver --input examples/example1.txt --k 2

# With increased trials for better quality (10x more trials)
./target/release/approx-solver --input examples/example1.txt --k 2 --trials-multiplier 10

# With significantly more trials (100x) for harder problems
./target/release/approx-solver --input examples/example6_large.txt --k 5 -t 100
```

### Command Line Arguments

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

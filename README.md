# Minimal k-Isomorphic Subgraph Extension

A Rust implementation for solving the Minimal k-Isomorphic Subgraph Extension problem.

## Problem Statement

Given a graph H and a pattern P, we need to find a minimal extension H′ such that it includes a set of k distinct mappings:

S = {f₁, f₂, ..., fₖ}

where each fᵢ is an isomorphic mapping from the pattern P to a subgraph in H′.

The goal is to find the smallest possible H′ that contains at least k distinct isomorphic embeddings of the pattern P.

## Project Structure

This project provides two solution approaches:

- **Exact Solution** (`exact-solver`): Computes the optimal minimal extension
- **Approximation Solution** (`approx-solver`): Provides an approximate solution with better performance

## Building

```bash
cargo build
```

## Running

### Exact Solver
```bash
cargo run --bin exact-solver
```

### Approximation Solver
```bash
cargo run --bin approx-solver
```

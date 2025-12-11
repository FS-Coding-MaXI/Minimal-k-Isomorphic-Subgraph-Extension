use clap::Parser;
use itertools::Itertools;
use minimal_k_isomorphic_subgraph_extension::{
    cost::{calculate_edge_map, calculate_total_cost},
    mapping::find_all_mappings,
    parser::parse_input_file,
    utils::num_combinations,
    Graph, Mapping,
};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Type alias for edge map: (source, target) -> edge count
type EdgeMap = HashMap<(usize, usize), usize>;

/// Type alias for the result of the exact algorithm
type SolutionResult = (usize, EdgeMap, Vec<Mapping>);

/// Exact Solver for Minimal k-Isomorphic Subgraph Extension
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the input file containing graph descriptions
    #[arg(short, long)]
    input: PathBuf,

    /// Number of distinct isomorphic mappings required (k)
    #[arg(short, long)]
    k: usize,
}

/// Main exact algorithm implementation
fn exact_minimal_k_extension(g: &Graph, h: &Graph, k: usize) -> Option<SolutionResult> {
    println!("Finding all possible mappings from G to H...");
    let all_mappings = find_all_mappings(g, h);

    println!("Found {} total mappings", all_mappings.len());

    if all_mappings.len() < k {
        println!(
            "Error: Not enough mappings found. Need {}, but only found {}",
            k,
            all_mappings.len()
        );
        return None;
    }

    println!("Evaluating all {}-combinations of mappings...", k);
    let total_combinations = num_combinations(all_mappings.len(), k);
    println!("Total combinations to evaluate: {}", total_combinations);

    let search_start = std::time::Instant::now();

    // Use separate Mutex for just the best cost to minimize lock contention
    let best_cost: Mutex<usize> = Mutex::new(usize::MAX);
    let best_result: Mutex<Option<(EdgeMap, Vec<Mapping>)>> = Mutex::new(None);

    // Parallel iteration over all k-combinations of mappings
    all_mappings
        .iter()
        .combinations(k)
        .par_bridge()
        .for_each(|combination| {
            let edge_map = calculate_edge_map(g, h, &combination);
            let total_cost = calculate_total_cost(&edge_map);

            // Quick check with minimal locking
            {
                let current_best = best_cost.lock().unwrap();
                if total_cost >= *current_best {
                    return; // Not better, skip
                }
            }

            // Found a better solution, update both cost and result
            {
                let mut cost_guard = best_cost.lock().unwrap();
                if total_cost < *cost_guard {
                    *cost_guard = total_cost;
                    drop(cost_guard); // Release cost lock before locking result

                    let mappings = combination.iter().map(|&m| m.clone()).collect();
                    let mut result_guard = best_result.lock().unwrap();
                    *result_guard = Some((edge_map, mappings));
                }
            }
        });

    let search_elapsed = search_start.elapsed();
    println!("Finished evaluating all combinations");
    println!("Search time: {:.3}s", search_elapsed.as_secs_f64());

    let final_cost = best_cost.into_inner().unwrap();
    let final_result = best_result.into_inner().unwrap();

    if let Some((optimal_edge_set, optimal_mappings)) = final_result {
        return Some((final_cost, optimal_edge_set, optimal_mappings));
    }

    None
}

fn main() {
    let args = Args::parse();

    println!("Exact Solver for Minimal k-Isomorphic Subgraph Extension");
    println!("==========================================================");
    println!();

    // Parse input graphs using nom-based parser
    let (g, h) = match parse_input_file(&args.input) {
        Ok(graphs) => graphs,
        Err(e) => {
            eprintln!("Error parsing input file: {}", e);
            std::process::exit(1);
        }
    };

    println!("Graph G (pattern): {} vertices", g.num_vertices());
    println!("Graph H (host): {} vertices", h.num_vertices());
    println!("Required distinct mappings (k): {}", args.k);
    println!();

    // Display adjacency matrices
    println!("Graph G adjacency matrix:");
    for row in &g.adj {
        println!("  {:?}", row);
    }
    println!();

    println!("Graph H adjacency matrix:");
    for row in &h.adj {
        println!("  {:?}", row);
    }
    println!();

    // Run exact algorithm
    println!("Running exact algorithm...");
    let start_time = std::time::Instant::now();

    match exact_minimal_k_extension(&g, &h, args.k) {
        Some((cost, edge_set, mappings)) => {
            let elapsed = start_time.elapsed();

            println!();
            println!("==========================================================");
            println!("OPTIMAL SOLUTION FOUND");
            println!("==========================================================");
            println!("Minimal total cost: {}", cost);
            println!("Computation time: {:.3} ms", elapsed.as_millis());
            println!("Computation time: {:.3} ns", elapsed.as_nanos());
            println!();

            println!("Adjacency matrix of edges to add to H:");
            let n = h.num_vertices();
            let mut add_matrix = vec![vec![0usize; n]; n];
            for ((u, v), weight) in &edge_set {
                add_matrix[*u][*v] = *weight;
            }
            for row in add_matrix {
                println!("  {:?}", row);
            }
            println!();

            println!("Optimal set of {} mappings:", args.k);
            for (i, mapping) in mappings.iter().enumerate() {
                println!("  Mapping {}: {:?}", i + 1, mapping);
            }
        }
        None => {
            println!();
            println!("No solution found. The host graph H is too small to contain {} distinct embeddings of G.", args.k);
            std::process::exit(1);
        }
    }
}

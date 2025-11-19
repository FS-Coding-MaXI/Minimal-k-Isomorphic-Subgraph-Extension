use clap::Parser;
use minimal_k_isomorphic_subgraph_extension::{
    Graph, Mapping,
    parser::parse_input_file,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use rand::seq::SliceRandom;
use rand::thread_rng;

/// Type alias for edge map: (source, target) -> edge count
type EdgeMap = HashMap<(usize, usize), usize>;

/// Approximation Solver for Minimal k-Isomorphic Subgraph Extension
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the input file containing graph descriptions
    #[arg(short, long)]
    input: PathBuf,

    /// Number of distinct isomorphic mappings required (k)
    #[arg(short, long)]
    k: usize,

    /// Trials multiplier (default: 1). Number of trials = n₁ × n₂ × multiplier
    #[arg(short = 't', long, default_value_t = 1)]
    trials_multiplier: usize,
}

/// Calculate local cost of mapping vertex u_i to v_j given current partial mapping
fn calculate_local_cost(
    u_i: usize,
    v_j: usize,
    g: &Graph,
    h_prime: &Graph,
    mapping: &HashMap<usize, usize>,
) -> usize {
    let mut cost = 0;
    
    // Check edges from u_i to all already-mapped vertices
    for (&u_mapped, &v_mapped) in mapping.iter() {
        // Edge from u_i to u_mapped in G
        let g_edge = g.get_edge(u_i, u_mapped);
        if g_edge > 0 {
            let h_edge = h_prime.get_edge(v_j, v_mapped);
            cost += g_edge.saturating_sub(h_edge);
        }
        
        // Edge from u_mapped to u_i in G
        let g_edge_rev = g.get_edge(u_mapped, u_i);
        if g_edge_rev > 0 {
            let h_edge_rev = h_prime.get_edge(v_mapped, v_j);
            cost += g_edge_rev.saturating_sub(h_edge_rev);
        }
    }
    
    cost
}

/// Apply extension to H' by updating edge weights according to the mapping
fn apply_extension(h_prime: &mut Graph, g: &Graph, mapping: &Mapping) {
    for u in 0..g.num_vertices() {
        for v in 0..g.num_vertices() {
            let x = mapping[u];
            let y = mapping[v];
            let required = g.get_edge(u, v);
            
            if required > h_prime.get_edge(x, y) {
                h_prime.adj[x][y] = required;
            }
        }
    }
}

/// Find approximately best mapping using randomized greedy approach
fn approximate_best_mapping(
    g: &Graph,
    h_prime: &Graph,
    used_mappings: &HashSet<Vec<usize>>,
    trials_multiplier: usize,
) -> Option<(Mapping, EdgeMap)> {
    let n_g = g.num_vertices();
    let n_h = h_prime.num_vertices();
    let t = n_g * n_h * trials_multiplier; // Number of trials
    
    let mut min_global_cost = usize::MAX;
    let mut best_global_mapping: Option<Mapping> = None;
    let mut best_edges_to_add = EdgeMap::new();
    
    let mut rng = thread_rng();
    
    for _ in 0..t {
        let mut mapping_map: HashMap<usize, usize> = HashMap::new();
        let mut edges_to_add = EdgeMap::new();
        
        // Random initial vertex mapping
        let g_vertices: Vec<usize> = (0..n_g).collect();
        let h_vertices: Vec<usize> = (0..n_h).collect();
        
        let u_start = g_vertices.choose(&mut rng).copied().unwrap();
        let v_start = h_vertices.choose(&mut rng).copied().unwrap();
        mapping_map.insert(u_start, v_start);
        
        let mut used_h_vertices = HashSet::new();
        used_h_vertices.insert(v_start);
        
        // Greedily map remaining vertices
        for u_i in 0..n_g {
            if mapping_map.contains_key(&u_i) {
                continue;
            }
            
            let mut min_local_cost = usize::MAX;
            let mut best_v_j = None;
            
            for v_j in 0..n_h {
                if used_h_vertices.contains(&v_j) {
                    continue;
                }
                
                let local_cost = calculate_local_cost(u_i, v_j, g, h_prime, &mapping_map);
                
                if local_cost < min_local_cost {
                    min_local_cost = local_cost;
                    best_v_j = Some(v_j);
                }
            }
            
            if let Some(v_j) = best_v_j {
                mapping_map.insert(u_i, v_j);
                used_h_vertices.insert(v_j);
            } else {
                break; // No more available vertices in H
            }
        }
        
        // Check if we got a complete, unique mapping
        if mapping_map.len() == n_g {
            let mapping_vec: Vec<usize> = (0..n_g).map(|i| mapping_map[&i]).collect();
            
            if used_mappings.contains(&mapping_vec) {
                continue; // Skip if already used
            }
            
            // Calculate total cost for this mapping
            let mut current_cost = 0;
            for u in 0..n_g {
                for v in 0..n_g {
                    let g_edge_count = g.get_edge(u, v);
                    if g_edge_count > 0 {
                        let x = mapping_vec[u];
                        let y = mapping_vec[v];
                        let h_edge_count = h_prime.get_edge(x, y);
                        let needed = g_edge_count.saturating_sub(h_edge_count);
                        
                        if needed > 0 {
                            edges_to_add.insert((x, y), needed);
                            current_cost += needed;
                        }
                    }
                }
            }
            
            if current_cost < min_global_cost {
                min_global_cost = current_cost;
                best_global_mapping = Some(mapping_vec);
                best_edges_to_add = edges_to_add.clone();
            }
        }
    }
    
    best_global_mapping.map(|m| (m, best_edges_to_add))
}

/// Sequential greedy extension for k subgraphs
fn sequential_greedy_extension(
    g: &Graph,
    h: &Graph,
    k: usize,
    trials_multiplier: usize,
) -> Option<(usize, EdgeMap, Vec<Mapping>)> {
    let mut h_prime = h.clone();
    let mut used_mappings = HashSet::new();
    let mut minimal_extension = EdgeMap::new();
    let mut all_mappings = Vec::new();
    
    let total_trials = g.num_vertices() * h.num_vertices() * trials_multiplier;
    println!("Finding {} distinct mappings using approximation algorithm...", k);
    println!("Trials per mapping: {} (n₁ × n₂ × {})", total_trials, trials_multiplier);
    
    for i in 1..=k {
        println!("Finding mapping {}/{}...", i, k);
        
        match approximate_best_mapping(g, &h_prime, &used_mappings, trials_multiplier) {
            Some((best_mapping, edges_to_add)) => {
                // Merge edges_to_add into minimal_extension (taking maximum)
                for ((x, y), weight) in edges_to_add.iter() {
                    let current = minimal_extension.get(&(*x, *y)).copied().unwrap_or(0);
                    if *weight > current {
                        minimal_extension.insert((*x, *y), *weight);
                    }
                }
                
                // Apply extension to H'
                apply_extension(&mut h_prime, g, &best_mapping);
                
                // Mark mapping as used
                used_mappings.insert(best_mapping.clone());
                all_mappings.push(best_mapping);
            }
            None => {
                println!("Failed to find mapping {}/{}", i, k);
                return None;
            }
        }
    }
    
    // Calculate total cost
    let total_cost: usize = minimal_extension.values().sum();
    
    Some((total_cost, minimal_extension, all_mappings))
}

fn main() {
    let args = Args::parse();

    println!("Approximation Solver for Minimal k-Isomorphic Subgraph Extension");
    println!("=================================================================");
    println!();

    // Parse input graphs
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
    println!("Trials multiplier: {}", args.trials_multiplier);
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

    // Run approximation algorithm
    println!("Running approximation algorithm...");
    let start_time = std::time::Instant::now();

    match sequential_greedy_extension(&g, &h, args.k, args.trials_multiplier) {
        Some((cost, edge_set, mappings)) => {
            let elapsed = start_time.elapsed();
            
            println!();
            println!("=================================================================");
            println!("APPROXIMATE SOLUTION FOUND");
            println!("=================================================================");
            println!("Total cost: {}", cost);
            println!("Computation time: {:.3}s", elapsed.as_secs_f64());
            println!();
            
            println!("Edges to add to H:");
            if edge_set.is_empty() {
                println!("  (no edges needed - H already contains k distinct embeddings of G)");
            } else {
                let mut edges: Vec<_> = edge_set.iter().collect();
                edges.sort_by_key(|(k, _)| *k);
                for ((u, v), weight) in edges {
                    println!("  Edge ({} -> {}): add {} edge(s)", u, v, weight);
                }
            }
            println!();

            println!("Found set of {} mappings:", args.k);
            for (i, mapping) in mappings.iter().enumerate() {
                println!("  Mapping {}: {:?}", i + 1, mapping);
            }
        }
        None => {
            println!();
            println!("Failed to find {} distinct embeddings of G in H.", args.k);
            std::process::exit(1);
        }
    }
}

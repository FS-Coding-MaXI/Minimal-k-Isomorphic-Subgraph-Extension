use clap::Parser;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

/// Input Generator for Minimal k-Isomorphic Subgraph Extension problem instances.
///
/// This tool produces an input file containing two directed multigraphs (G and H)
/// in the format expected by the exact and approximation solvers:
///
/// <n1>
/// <adjacency matrix for G: n1 rows of n1 space-separated integers>
/// <n2>
/// <adjacency matrix for H: n2 rows of n2 space-separated integers>
///
/// Design goals for "interesting" test instances:
/// - G has moderate density with occasional multiedges
/// - H is larger (n2 > n1) with lower density
/// - A randomly chosen injective mapping of G into H is "partially satisfied":
///     * Some edges of G already exist with enough multiplicity under that mapping
///     * Some edges require extension (added edges) to realize the mapping
/// - Remaining vertices in H (those not used by the embedded mapping) receive light noise
///
/// You can tune parameters to control difficulty and sparsity.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Generate random test instances for Minimal k-Isomorphic Subgraph Extension"
)]
struct Args {
    /// Number of vertices in pattern graph G (n1)
    #[arg(long)]
    n1: usize,

    /// Number of vertices in host graph H (n2), must be > n1
    #[arg(long)]
    n2: usize,

    /// Density (probability of an edge) for G (excluding self-loops)
    #[arg(long, default_value_t = 0.35)]
    density_g: f64,

    /// Density (probability of an edge) for H base graph (excluding self-loops)
    #[arg(long, default_value_t = 0.20)]
    density_h: f64,

    /// Probability that an existing edge (when generated) becomes a multiedge
    #[arg(long, default_value_t = 0.15)]
    multiedge_prob: f64,

    /// Maximum multiplicity for a multiedge (uniformly sampled in [2, max])
    #[arg(long, default_value_t = 4)]
    max_multiedge: usize,

    /// Fraction of G's edges to "embed" strongly into H (already satisfied)
    #[arg(long, default_value_t = 0.40)]
    embed_strength: f64,

    /// Fraction of G's edges to force as "under-satisfied" (H multiplicity < G multiplicity)
    #[arg(long, default_value_t = 0.35)]
    deficit_strength: f64,

    /// Add weak random noise edges among unused H vertices (0..noise_max)
    #[arg(long, default_value_t = true)]
    noise: bool,

    /// Maximum random edge multiplicity for noise among unused H vertices
    #[arg(long, default_value_t = 1)]
    noise_max: usize,

    /// Random seed (if omitted, uses entropy)
    #[arg(long)]
    seed: Option<u64>,

    /// Output file path to write the raw instance (mandatory)
    #[arg(long)]
    output: PathBuf,
    // (header and allow_self_loops flags removed; stats printed to stdout only)
    // (self loops disabled; generator omits them)
    // RESERVED
    // RESERVED
    // RESERVED
    // RESERVED
    // RESERVED
}

/// Generate a random edge count (>=1) possibly becoming a multiedge
fn random_edge_count<R: Rng>(rng: &mut R, multiedge_prob: f64, max_multiedge: usize) -> usize {
    if max_multiedge < 2 || rng.gen::<f64>() >= multiedge_prob {
        1
    } else {
        // Uniform between 2..=max_multiedge
        rng.gen_range(2..=max_multiedge)
    }
}

/// Build a random directed multigraph adjacency matrix
fn generate_graph<R: Rng>(
    n: usize,
    density: f64,
    multiedge_prob: f64,
    max_multiedge: usize,
    // removed allow_self_loops
    rng: &mut R,
) -> Vec<Vec<usize>> {
    let mut adj = vec![vec![0usize; n]; n];
    for (i, row) in adj.iter_mut().enumerate() {
        for (j, val) in row.iter_mut().enumerate() {
            if i == j {
                continue;
            }
            if rng.gen::<f64>() < density {
                *val = random_edge_count(rng, multiedge_prob, max_multiedge);
            }
        }
    }
    adj
}

/// Select a random injective mapping from G's vertices into distinct vertices of H
fn random_injective_mapping<R: Rng>(n1: usize, n2: usize, rng: &mut R) -> Vec<usize> {
    let mut pool: Vec<usize> = (0..n2).collect();
    // Fisher-Yates shuffle then take first n1
    for i in (1..n2).rev() {
        let j = rng.gen_range(0..=i);
        pool.swap(i, j);
    }
    pool.into_iter().take(n1).collect()
}

/// Apply embedding adjustments to H based on G and chosen mapping.
///
/// For a subset of edges in G determined by embed_strength:
///   Ensure H[m[i]][m[j]] >= G[i][j]
///
/// For a subset determined by deficit_strength (distinct from embed set):
///   Ensure 0 <= H[m[i]][m[j]] < G[i][j] (force deficit)
///
/// Edges not in either set are left unchanged (may or may not satisfy).
fn adjust_embedding<R: Rng>(
    g: &[Vec<usize>],
    h: &mut [Vec<usize>],
    mapping: &[usize],
    embed_strength: f64,
    deficit_strength: f64,
    rng: &mut R,
) {
    // Collect list of existing edges in G (i,j) where g[i][j] > 0
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (i, row) in g.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            if val > 0 {
                edges.push((i, j));
            }
        }
    }
    if edges.is_empty() {
        return;
    }

    // Shuffle edges
    for i in (1..edges.len()).rev() {
        let j = rng.gen_range(0..=i);
        edges.swap(i, j);
    }

    let embed_count = ((edges.len() as f64) * embed_strength).round() as usize;
    let deficit_count = ((edges.len() as f64) * deficit_strength).round() as usize;

    let embed_slice_end = embed_count.min(edges.len());
    let deficit_slice_end = (embed_count + deficit_count).min(edges.len());

    let (embed_edges, rest) = edges.split_at(embed_slice_end);
    let (deficit_edges, _others) = rest.split_at(deficit_slice_end.saturating_sub(embed_slice_end));

    // Apply embeddings (satisfied edges)
    for &(i, j) in embed_edges {
        let hi = mapping[i];
        let hj = mapping[j];
        if h[hi][hj] < g[i][j] {
            h[hi][hj] = g[i][j]; // satisfy exactly
        } else {
            // Possibly increase multiplicity slightly to create alternative mapping complexity
            if rng.gen::<f64>() < 0.10 {
                h[hi][hj] = h[hi][hj].max(g[i][j]);
            }
        }
    }

    // Apply deficits (under-satisfied edges)
    for &(i, j) in deficit_edges {
        let hi = mapping[i];
        let hj = mapping[j];
        let g_req = g[i][j];
        let current = h[hi][hj];
        if current >= g_req {
            // Force a deficit by lowering (only if >0)
            if current > 0 {
                h[hi][hj] = rng.gen_range(0..g_req);
            } else {
                // Keep at 0 (needs full addition)
                h[hi][hj] = 0;
            }
        } else {
            // Already a deficit; optionally make it "worse"
            if rng.gen::<f64>() < 0.25 {
                h[hi][hj] = current.saturating_sub(1);
            }
        }
    }
}

/// Inject light noise among unused H vertices (those not in the chosen mapping).
fn add_noise_among_unused<R: Rng>(
    h: &mut [Vec<usize>],
    unused: &[usize],
    noise_max: usize,
    rng: &mut R,
    // removed allow_self_loops
) {
    if noise_max == 0 || unused.is_empty() {
        return;
    }
    for &u in unused {
        for &v in unused {
            if u == v {
                continue;
            }
            // Small probability of a random edge
            if rng.gen::<f64>() < 0.08 {
                let val = rng.gen_range(0..=noise_max);
                if val > h[u][v] {
                    h[u][v] = val;
                }
            }
        }
    }
}

/// Write adjacency matrix as space-separated rows
fn write_matrix<W: Write>(writer: &mut W, adj: &[Vec<usize>]) -> io::Result<()> {
    for row in adj {
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                write!(writer, " ")?;
            }
            write!(writer, "{}", val)?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

// Header/stats functionality removed: stats now printed only to stdout,
// and never written into the generated file.

fn main() -> io::Result<()> {
    let args = Args::parse();

    if args.n1 == 0 || args.n2 == 0 {
        eprintln!("Error: n1 and n2 must be positive.");
        std::process::exit(1);
    }
    if args.n1 >= args.n2 {
        eprintln!("Error: n1 must be strictly less than n2.");
        std::process::exit(1);
    }
    if !(0.0..=1.0).contains(&args.density_g) || !(0.0..=1.0).contains(&args.density_h) {
        eprintln!("Error: densities must be in [0,1].");
        std::process::exit(1);
    }
    if !(0.0..=1.0).contains(&args.multiedge_prob) {
        eprintln!("Error: multiedge_prob must be in [0,1].");
        std::process::exit(1);
    }
    if !(0.0..=1.0).contains(&args.embed_strength) || !(0.0..=1.0).contains(&args.deficit_strength)
    {
        eprintln!("Error: embed_strength and deficit_strength must be in [0,1].");
        std::process::exit(1);
    }
    if args.max_multiedge < 2 && args.multiedge_prob > 0.0 {
        eprintln!("Warning: max_multiedge < 2 makes multiedge_prob ineffective.");
    }

    // Initialize RNG
    let mut rng: StdRng = match args.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => {
            // Use system entropy
            let seed: u64 = rand::thread_rng().gen();
            StdRng::seed_from_u64(seed)
        }
    };

    // Generate G
    let g_adj = generate_graph(
        args.n1,
        args.density_g,
        args.multiedge_prob,
        args.max_multiedge,
        &mut rng,
    );

    // Generate base H
    let mut h_adj = generate_graph(
        args.n2,
        args.density_h,
        args.multiedge_prob,
        args.max_multiedge,
        &mut rng,
    );

    // Choose injective mapping
    let mapping = random_injective_mapping(args.n1, args.n2, &mut rng);

    // Adjust embedding to create satisfied and deficit edges
    adjust_embedding(
        &g_adj,
        &mut h_adj,
        &mapping,
        args.embed_strength,
        args.deficit_strength,
        &mut rng,
    );

    // Add light noise among unused vertices
    if args.noise {
        let used: std::collections::HashSet<usize> = mapping.iter().copied().collect();
        let unused: Vec<usize> = (0..args.n2).filter(|v| !used.contains(v)).collect();
        add_noise_among_unused(&mut h_adj, &unused, args.noise_max, &mut rng);
    }

    // Prepare writer (always write raw instance without header)
    let mut writer = File::create(&args.output)?;

    // Print stats to stdout (not into the file)
    {
        let g_edges: usize = g_adj
            .iter()
            .flat_map(|r| r.iter())
            .filter(|&&c| c > 0)
            .count();
        let h_edges: usize = h_adj
            .iter()
            .flat_map(|r| r.iter())
            .filter(|&&c| c > 0)
            .count();
        println!("Generated instance:");
        println!("  n1 = {}", args.n1);
        println!("  n2 = {}", args.n2);
        println!("  density_g = {:.3}", args.density_g);
        println!("  density_h = {:.3}", args.density_h);
        println!("  multiedge_prob = {:.3}", args.multiedge_prob);
        println!("  max_multiedge = {}", args.max_multiedge);
        println!("  embed_strength = {:.3}", args.embed_strength);
        println!("  deficit_strength = {:.3}", args.deficit_strength);
        println!("  noise = {}, noise_max = {}", args.noise, args.noise_max);
        if let Some(seed) = args.seed {
            println!("  seed = {}", seed);
        }
        println!("  mapping (G->H): {:?}", mapping);
        println!("  non-zero edges: G = {}, H = {}", g_edges, h_edges);
        println!("  output file: {:?}", args.output);
    }

    // Emit final instance
    writeln!(writer, "{}", args.n1)?;
    write_matrix(&mut writer, &g_adj)?;
    writeln!(writer)?;
    writeln!(writer, "{}", args.n2)?;
    write_matrix(&mut writer, &h_adj)?;

    // Flush explicitly
    writer.flush()?;

    Ok(())
}

use crate::{Graph, Mapping};
use std::collections::HashMap;

/// Calculate the edge map needed to implement a set of mappings
/// Returns a HashMap of (u, v) -> weight representing edges to add
pub fn calculate_edge_map(
    g: &Graph,
    h: &Graph,
    mappings: &[&Mapping],
) -> HashMap<(usize, usize), usize> {
    let mut edge_map = HashMap::new();

    for mapping in mappings {
        for u in 0..g.num_vertices() {
            for v in 0..g.num_vertices() {
                let g_edge_count = g.get_edge(u, v);
                if g_edge_count > 0 {
                    let x = mapping[u];
                    let y = mapping[v];
                    let h_edge_count = h.get_edge(x, y);
                    let needed = g_edge_count.saturating_sub(h_edge_count);

                    // Take maximum across all mappings for this edge
                    let current = edge_map.get(&(x, y)).copied().unwrap_or(0);
                    if needed > current {
                        edge_map.insert((x, y), needed);
                    }
                }
            }
        }
    }

    edge_map
}

/// Calculate total cost (sum of all edge weights in the edge map)
pub fn calculate_total_cost(edge_map: &HashMap<(usize, usize), usize>) -> usize {
    edge_map.values().sum()
}

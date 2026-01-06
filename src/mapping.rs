use crate::{Graph, Mapping};

/// Find all possible injective mappings from pattern graph G to host graph H
pub fn find_all_mappings(g: &Graph, h: &Graph) -> Vec<Mapping> {
    let n_g = g.num_vertices();
    let n_h = h.num_vertices();

    if n_g > n_h {
        return vec![]; // No valid mappings possible
    }

    let mut all_mappings = Vec::new();
    let mut current_mapping = vec![0; n_g];
    let mut used_vh = vec![false; n_h];

    backtrack(
        0,
        n_g,
        n_h,
        &mut current_mapping,
        &mut used_vh,
        &mut all_mappings,
    );

    all_mappings
}

/// Recursive backtracking to enumerate all injective mappings
fn backtrack(
    vertex_idx: usize,
    n_g: usize,
    n_h: usize,
    current_mapping: &mut Vec<usize>,
    used_vh: &mut Vec<bool>,
    all_mappings: &mut Vec<Mapping>,
) {
    if vertex_idx == n_g {
        // Complete mapping found
        all_mappings.push(current_mapping.clone());
        return;
    }

    // Try mapping current vertex to each unused vertex in H
    for v in 0..n_h {
        if !used_vh[v] {
            current_mapping[vertex_idx] = v;
            used_vh[v] = true;
            backtrack(
                vertex_idx + 1,
                n_g,
                n_h,
                current_mapping,
                used_vh,
                all_mappings,
            );
            used_vh[v] = false;
        }
    }
}

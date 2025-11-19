/// Represents a directed multigraph with adjacency matrix
#[derive(Debug, Clone)]
pub struct Graph {
    /// Number of vertices
    pub n: usize,
    /// Adjacency matrix: adj[i][j] = number of edges from vertex i to vertex j
    pub adj: Vec<Vec<usize>>,
}

impl Graph {
    pub fn new(n: usize) -> Self {
        Graph {
            n,
            adj: vec![vec![0; n]; n],
        }
    }

    pub fn from_adjacency_matrix(adj: Vec<Vec<usize>>) -> Self {
        let n = adj.len();
        Graph { n, adj }
    }

    pub fn num_vertices(&self) -> usize {
        self.n
    }

    pub fn get_edge(&self, u: usize, v: usize) -> usize {
        self.adj[u][v]
    }
}

/// Represents an injective mapping from pattern graph to host graph
pub type Mapping = Vec<usize>;

// Module declarations
pub mod parser;
pub mod mapping;
pub mod cost;
pub mod utils;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_creation() {
        let g = Graph::new(3);
        assert_eq!(g.num_vertices(), 3);
        assert_eq!(g.get_edge(0, 0), 0);
    }

    #[test]
    fn test_parse_simple_graph() {
        let input = "2\n0 1\n0 0\n\n2\n0 1\n0 0\n";
        let result = parser::parse_two_graphs(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_mappings() {
        let g = Graph::from_adjacency_matrix(vec![vec![0, 1], vec![0, 0]]);
        let h = Graph::from_adjacency_matrix(vec![vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]]);
        let mappings = mapping::find_all_mappings(&g, &h);
        assert_eq!(mappings.len(), 6); // P(3, 2) = 6
    }

    #[test]
    fn test_combinations() {
        assert_eq!(utils::num_combinations(5, 2), 10);
        assert_eq!(utils::num_combinations(4, 4), 1);
        assert_eq!(utils::num_combinations(3, 0), 1);
    }
}

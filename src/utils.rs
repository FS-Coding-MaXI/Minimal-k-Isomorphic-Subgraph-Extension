/// Calculate number of combinations C(n, k) without overflow
pub fn num_combinations(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    if k == 0 || k == n {
        return 1;
    }
    
    let k = k.min(n - k); // Optimization: C(n,k) = C(n,n-k)
    let mut result = 1usize;
    
    for i in 0..k {
        result = result.saturating_mul(n - i) / (i + 1);
    }
    
    result
}

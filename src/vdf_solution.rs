use primitive_types::U256;
use rand_mt::Mt19937GenRand64;
use std::{
    iter,
    time::{Duration, Instant},
};

pub const GRAPH_SIZE: u16 = 2008;

pub struct HCGraphUtil {
    start_time: Instant,
    vdf_bailout: u64,
}

impl HCGraphUtil {
    pub fn new(vdf_bailout: Option<u64>) -> Self {
        let bailout_timer: u64 = match vdf_bailout {
            Some(timer) => timer,
            None => 1000, // default to 1 second
        };
        HCGraphUtil {
            start_time: Instant::now(),
            vdf_bailout: bailout_timer,
        }
    }

    fn hex_to_u64(&self, hex_string: &str) -> u64 {
        u64::from_str_radix(hex_string, 16).expect("Failed to convert hex to u64")
    }

    fn read_le_u64(&self, bytes: &[u8]) -> u64 {
        let arr: [u8; 8] = bytes[..8].try_into().expect("Slice with incorrect length");
        u64::from_le_bytes(arr)
    }

    fn get_u64(&self, data: &[u8], pos: usize) -> u64 {
        self.read_le_u64(&data[pos * 8..(pos + 1) * 8])
    }

    fn extract_seed_from_hash(&self, hash: &U256) -> u64 {
        let bytes = hash.to_little_endian();
        self.get_u64(&bytes, 0)
    }

    fn get_grid_size_v2(&self, hash: &U256) -> u16 {
        let hash_hex = format!("{:064x}", hash);
        let grid_size_segment = &hash_hex[0..8];
        let grid_size: u64 = self.hex_to_u64(grid_size_segment);

        let min_grid_size = 2000u64;
        let max_grid_size = GRAPH_SIZE as u64;

        let mut grid_size_final = min_grid_size + (grid_size % (max_grid_size - min_grid_size));
        if grid_size_final > max_grid_size {
            grid_size_final = max_grid_size;
        }
        grid_size_final as u16
    }

    fn generate_graph_v2(&self, hash: &U256, grid_size: u16) -> Vec<bool> {
        let grid_size = grid_size as usize;
        let mut graph = vec![false; grid_size * grid_size];
        let num_edges = (grid_size * (grid_size - 1)) / 2;
        let bits_needed = num_edges;

        let seed = self.extract_seed_from_hash(hash);
        let mut prng = Mt19937GenRand64::from(seed.to_le_bytes());

        let mut bit_stream = Vec::with_capacity(bits_needed);

        while bit_stream.len() < bits_needed {
            let random_bits_32: u32 = (prng.next_u64() & 0xFFFFFFFF) as u32;
            for j in (0..32).rev() {
                if bit_stream.len() >= bits_needed {
                    break;
                }
                let bit = ((random_bits_32 >> j) & 1) == 1;
                bit_stream.push(bit);
            }
        }

        let mut bit_index = 0;
        for i in 0..grid_size {
            for j in (i + 1)..grid_size {
                let edge_exists = bit_stream[bit_index];
                bit_index += 1;
                graph[grid_size * i + j] = edge_exists;
                graph[grid_size * j + i] = edge_exists;
            }
        }

        graph
    }

    fn _opt(&self, hash: &U256, grid_size: u16) -> Vec<bool> {
        let grid_size = grid_size as usize;
        let mut graph = vec![false; grid_size * grid_size];
        let num_edges = (grid_size * (grid_size - 1)) / 2;

        let seed = self.extract_seed_from_hash(hash);
        let mut prng = Mt19937GenRand64::from(seed.to_le_bytes());

        // 位流生成作为一次性流，而不是存储在一个容器中
        let mut bit_iterator = iter::from_fn(|| {
            let random_bits_32: u32 = (prng.next_u64() & 0xFFFFFFFF) as u32;
            Some(random_bits_32)
        })
        .flat_map(|bits| (0..32).rev().map(move |j| ((bits >> j) & 1) == 1))
        .take(num_edges);

        // 遍历整个图只为抓取必要的位
        for i in 0..grid_size {
            for j in (i + 1)..grid_size {
                if let Some(edge_exists) = bit_iterator.next() {
                    graph[grid_size * i + j] = edge_exists;
                    graph[grid_size * j + i] = edge_exists;
                }
            }
        }

        graph
    }

    fn is_safe(&self, v: u16, graph: &Vec<bool>, path: &[u16], pos: usize, grid_size: u16) -> bool {
        if !graph[(path[pos - 1] as usize) * (grid_size as usize - 1) + v as usize] {
            return false;
        }

        for i in 0..pos {
            if path[i] == v {
                return false;
            }
        }

        true
    }

    fn is_safe_vp(
        &self,
        v: u16,
        graph: &Vec<bool>,
        path: &[u16],
        pos: usize,
        grid_size: u16,
    ) -> bool {
        if pos == 0 || !graph[(path[pos - 1] as usize) * (grid_size as usize - 1) + v as usize] {
            return false;
        }

        for &node in &path[..pos] {
            if node == v {
                return false; // 如果已经访问过，返回 false
            }
        }

        true
    }

    fn hamiltonian_cycle_util(
        &mut self,
        graph: &Vec<bool>,
        path: &mut [u16],
        pos: usize,
        grid_size: u16,
    ) -> bool {
        let elapsed = self.start_time.elapsed();
        if elapsed > Duration::from_millis(self.vdf_bailout) {
            return false;
        }

        if pos == grid_size as usize {
            return graph[(path[pos - 1] as usize) * (grid_size as usize - 1) + path[0] as usize];
        }

        for v in 1..graph.len() as u16 {
            if self.is_safe(v, graph, path, pos, grid_size) {
                path[pos] = v;

                if self.hamiltonian_cycle_util(graph, path, pos + 1, grid_size) {
                    return true;
                }

                path[pos] = u16::MAX;
            }
        }

        false
    }

    fn hamiltonian_cycle_util_vp(
        &mut self,
        graph: &Vec<bool>,
        path: &mut [u16],
        visited: &mut Vec<bool>,
        pos: usize,
        grid_size: u16,
    ) -> bool {
        let elapsed = self.start_time.elapsed();
        if elapsed > Duration::from_millis(self.vdf_bailout) {
            return false;
        }

        if pos == grid_size as usize {
            return graph[(path[pos - 1] as usize) * (grid_size as usize - 1) + path[0] as usize];
        }

        for v in 1..grid_size as usize {
            if !visited[v] && self.is_safe_vp(v as u16, graph, path, pos, grid_size) {
                path[pos] = v as u16;
                visited[v] = true;

                if self.hamiltonian_cycle_util_vp(graph, path, visited, pos + 1, grid_size) {
                    return true;
                }

                path[pos] = u16::MAX;
                visited[v] = false;
            }
        }

        false
    }

    pub fn find_hamiltonian_cycle_v2(&mut self, graph_hash: U256) -> Vec<u16> {
        let grid_size = self.get_grid_size_v2(&graph_hash);
        let graph = self.generate_graph_v2(&graph_hash, grid_size);

        let mut path = vec![u16::MAX; graph.len()];
        path[0] = 0;
        self.start_time = Instant::now();

        if !self.hamiltonian_cycle_util(&graph, &mut path, 1, grid_size) {
            return vec![];
        }
        path
    }

    pub fn find_hamiltonian_cycle_vp(&mut self, graph_hash: U256) -> Vec<u16> {
        let grid_size = self.get_grid_size_v2(&graph_hash);
        let graph = self._opt(&graph_hash, grid_size);

        let mut path = vec![u16::MAX; grid_size as usize];
        path[0] = 0;
        let mut visited = vec![false; grid_size as usize];
        visited[0] = true;
        self.start_time = Instant::now();

        if !self.hamiltonian_cycle_util_vp(&graph, &mut path, &mut visited, 1, grid_size) {
            return vec![];
        }
        path
    }
}

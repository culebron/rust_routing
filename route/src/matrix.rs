
use std::cmp;
use osm_route::objects::{VertexId, Weight};
use osm_route::graph::Graph;

const WEIGHT_MAX: Weight = Weight(100000);

pub struct FloydWarshall {
	num_nodes: usize,
	matrix: Vec<Weight>,
}

impl FloydWarshall {
	pub fn new(num_nodes: usize) -> Self {
		// todo: move num_nodes initialization into prepare and prevent calling calc_path before
		// prepare
		FloydWarshall {
			num_nodes,
			matrix: vec![WEIGHT_MAX; num_nodes * num_nodes],
		}
	}

	pub fn prepare(&mut self, input_graph: &Graph) {
		assert_eq!(
			input_graph.get_num_vertice(),
			self.num_nodes,
			"input graph has invalid number of nodes"
		);
		let n = self.num_nodes;
		for v in input_graph.vertice.values() {
			for e in v.edges.iter() {
				self.matrix[e.v1 * n + e.v2] = e.weight;
			}
		}
		for k in 0..n {
			for i in 0..n {
				for j in 0..n {
					if i == j {
						self.matrix[i * n + j] = 0;
					}
					let weight_ik = self.matrix[i * n + k];
					let weight_kj = self.matrix[k * n + j];
					if weight_ik == WEIGHT_MAX || weight_kj == WEIGHT_MAX {
						continue;
					}
					let idx = i * n + j;
					self.matrix[idx] = cmp::min(self.matrix[idx], weight_ik + weight_kj)
				}
			}
		}
	}

	pub fn calc_weight(&self, source: NodeId, target: NodeId) -> Weight {
		return self.matrix[source * self.num_nodes + target];
	}
}

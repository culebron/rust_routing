use crate::{
	errors::{RoutingError, ok_or_nr},
	objects::{VertexId, Cost, VertexScore},
	graph::Graph
};
use geo::Point;
use std::{ collections::{HashMap, BinaryHeap}, error::Error};

#[derive(Debug, Clone)]
pub struct Isochrone {
	pub source: VertexId,
	pub source_geom: Point,
	pub distances: HashMap<VertexId, Cost>,
}

impl Isochrone {
	pub fn try_new(graph: &Graph, source: &VertexId, max_dist: Option<&Cost>) -> Result<Self, Box<dyn Error>> {
		let vertice = &graph.vertice;
		let max_dist = max_dist.unwrap_or(&Cost(0));
		let source_geom = vertice.get(source).ok_or(format!("source nid {:?} not in the graph", source.0))?.geom;
		let mut heap: BinaryHeap<VertexScore> = BinaryHeap::new();
		let start_vs = VertexScore::new(source.clone(), source.clone(), Cost(0), Cost(0));
		let mut iso = Self { source: source.clone(), source_geom, distances: HashMap::new() };

		heap.push(start_vs.clone());

		while !heap.is_empty() {
			let vs = heap.pop().unwrap();
			let current_id = vs.vid;
			if iso.distances.contains_key(&vs.vid) { continue; }

			iso.distances.insert(vs.vid.clone(), vs.cost_before);

			let vtx1 = vertice.get(&vs.vid).expect("node id not existing");

			for e in vtx1.edges.iter() {
				let other_id = e.v2;

				if !iso.distances.contains_key(&other_id) {
					let other_score = VertexScore::new(
						other_id, current_id,
						vs.cost_before + e.weight, Cost(0));

					// add to isochrone
					if max_dist > &other_score.cost_before || max_dist.0 == 0 {
						heap.push(other_score);
					}
				}
			}
		}

		Ok(iso)
	}

	pub fn estimate(&self, source: &VertexId, target: &VertexId) -> Cost {
		let source_dist = self.distances.get(source).expect(&format!("source must be existing in isochrone, got {} which is absent. iso source: {}", source.0, self.source.0));
		let target_dist = self.distances.get(target).expect(&format!("target must be existing in isochrone, got {} which is absent. iso source: {}", target.0, self.source.0));
		*source_dist - *target_dist
	}

	pub fn check(&self, vid: &VertexId) -> Result<&Cost, RoutingError> {
		ok_or_nr(self.distances.get(vid), &format!("vertex {} is not in the graph", vid.0))
	}
}

#[cfg(test)]
mod isochrone_tests {
	use super::*;
	use crate::objects::{Edge, Weight};
	use geo::LineString;

	#[test]
	fn test_isochrone() {
		//
		//	1---2---4---6---8---10
		//	    |   |   |   |
		// 	    3---5   7---9---11
		// all edge lengths are 2 (to exactly split the graph w/o less than/less than or equal issues)

		let mut g = Graph::new();
		let dummy_geom = LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]);
		let edges: Vec<(i64, i64)> = vec![
			(1, 2), (2, 3), (2, 4), (3, 5),
			(4, 6), (6, 7), (6, 8), (7, 9),
			(8, 9), (8, 10), (9, 11),
		];

		for (vid1, vid2) in edges.iter() {
			let (v1, v2) = (VertexId(*vid1), VertexId(*vid2));
			g.add_edge(Edge { v1, v2, weight: Weight(2), geom: dummy_geom.clone() }, true);
		}

		// UNLIMITED ISOCHRONE must have all vertice
		dbg!(format!("{:?}", g.vertice.len()));
		assert_eq!(g.vertice.len(), 11);

		let src = VertexId(1);
		let iso = Isochrone::try_new(&g, &src, None).unwrap();
		assert_eq!(iso.source, src);
		assert_eq!(iso.distances.len(), 11);

		for (i, d) in vec![0, 0, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6].iter().enumerate() {
			if i == 0 { continue; } // skip item 0
			let vid = VertexId((i) as i64);
			let (actual, expected) = (iso.distances.get(&vid).unwrap(), &Cost(*d * 2));
			dbg!(format!("COSTS {:?} {:?}", actual, expected));
			assert_eq!(actual, expected);
		}

		let iso2 = Isochrone::try_new(&g, &src, Some(&Cost(7))).unwrap();
		assert_eq!(iso2.distances.len(), 6);  // only 5 nodes will be there

	}
}

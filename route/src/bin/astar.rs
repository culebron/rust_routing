use rayon::prelude::*;
use indicatif::{ProgressBar, ProgressState, ProgressStyle, ProgressFinish};
use rand::{thread_rng, seq::SliceRandom};
use osm_route::{
	errors::RoutingError,
	objects::{VertexId, Cost, VertexScore, VisitedMap, TimeCheck},
	graph::{Graph, BidirPath},
	traits::{GraphPath, Router},
};
use geo::EuclideanDistance;
use std::{ collections::{BinaryHeap}, error::Error};


pub struct AstarRouter<'a> {
	pub graph: &'a Graph
}

impl<'a> Router for AstarRouter<'a> {
	type ResultPath = BidirPath<'a>;
	fn get_graph(&self) -> &Graph { self.graph }
	fn shortest_path(&self, source: &VertexId, target: &VertexId) -> Result<BidirPath<'a>, RoutingError> {
		let vertice = &self.graph.vertice;
		let mut forward_heap: BinaryHeap<VertexScore> = BinaryHeap::new();
		let mut backward_heap: BinaryHeap<VertexScore> = BinaryHeap::new();
		let mut forward_visited = VisitedMap::new();
		let mut backward_visited = VisitedMap::new();

		let start_vertex = vertice.get(source).unwrap();
		let end_vertex =   vertice.get(target).unwrap();

		let dist = start_vertex.geom.euclidean_distance(&end_vertex.geom) as i64;

		let vs2 = VertexScore::new(source.clone(), source.clone(), Cost(0), Cost(dist));
		forward_heap.push(vs2);
		let vs2 = VertexScore::new(target.clone(), target.clone(), Cost(0), Cost(dist));
		backward_heap.push(vs2);

		let mut visit_number:isize = 0;

		while !forward_heap.is_empty() && !backward_heap.is_empty() {
			let mut vs = forward_heap.pop().unwrap();
			vs.visit_number = visit_number;
			visit_number += 1;
			if forward_visited.contains_key(&vs.vid) {
				continue;
			}

			forward_visited.insert(vs.vid, vs.clone());
			if backward_visited.contains_key(&vs.vid) {
				return Ok(BidirPath { forward_visited, backward_visited, graph: &self.graph, meet_vertex: vs.vid });
			}
			let vtx1 = vertice.get(&vs.vid).expect("node id not existing");

			for e in vtx1.edges.iter() {
				let vtx2 = vertice.get(&e.v2).expect("other node not existing in vertice hashmap");


				if !forward_visited.contains_key(&vtx2.id) {
					let vs2 = VertexScore::new(
						vtx2.id, vs.vid.clone(),
						vs.cost_before + e.weight,
						Cost(vtx2.geom.euclidean_distance(&end_vertex.geom) as i64)
					);
					forward_heap.push(vs2);
				}
			}

			let mut vs = backward_heap.pop().unwrap();
			vs.visit_number = visit_number;
			visit_number += 1;
			if backward_visited.contains_key(&vs.vid) {
				continue;
			}

			backward_visited.insert(vs.vid, vs.clone());
			if forward_visited.contains_key(&vs.vid) {
				return Ok(BidirPath { forward_visited, backward_visited, graph: &self.graph, meet_vertex: vs.vid });
			}

			let vtx1 = vertice.get(&vs.vid).expect("node id not existing");

			for e in vtx1.edges.iter() {
				let vtx2 = vertice.get(&e.v2).expect("other node not existing in vertice hashmap");

				if !backward_visited.contains_key(&vtx2.id) {
					let vs2 = VertexScore::new(
						vtx2.id, vs.vid.clone(),
						vs.cost_before + e.weight,
						Cost(vtx2.geom.euclidean_distance(&start_vertex.geom) as i64)
					);
					backward_heap.push(vs2);
				}
			}
		}

		Err(RoutingError::NoRoute { msg: "no route".into() })
	}
}


fn run(path: &str) -> Result<(), Box<dyn Error>> {
	let mut tc = TimeCheck::new();
	let graph = Graph::from_path(path, true)?;
	let astar = AstarRouter { graph: &graph };
	println!("reading graph: {} s", tc.delta()?);

	let mut rng = thread_rng();
	let vids: Vec<&VertexId> = graph.vertice.keys().collect();

	println!("graph: {:?} vertice", graph.vertice.len());

	if cfg!(debug_assertions) {
		use osm_route::debug::debug_router;
		debug_router(&astar, "data/astar/")?;
	}
	else {
		let iterations = 1000u64;
		let iterrr = 0..iterations;
		let pbr = ProgressBar::new(iterations)
				.with_style(ProgressStyle::with_template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
				.unwrap()
				.with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
				.progress_chars("#>-"))
				.with_finish(ProgressFinish::Abandon)
				//.wrap_iter(iterrr)
				;

		let pairs: Vec<(&VertexId, &VertexId)> = iterrr.map(|_| {
			(*vids[..].choose(&mut rng).unwrap(), *vids[..].choose(&mut rng).unwrap())
		}).collect();

		let hops: Vec<Result<usize, RoutingError>> = pairs.par_iter().map(|(vid1, vid2)| {
			pbr.inc(1);
			let bdp = astar.shortest_path(&vid1, &vid2)?;
			Ok(bdp.forward_visited.len() + bdp.backward_visited.len())
		}).collect();

		let good_routes: Vec<usize> = hops.iter().filter(|v| v.is_ok()).map(|v| v.as_ref().unwrap().clone()).collect();
		println!("{:?}", good_routes);
		let mean:f64 = good_routes.clone().into_iter().map(|v| v as f64 / good_routes.len() as f64).sum();
		let mean_share = mean / graph.vertice.len() as f64 * 100.0;
		let bad_routes = hops.iter().filter(|v| v.is_err()).count();
		println!("mean: {:.2} ({:.2}%), bad routes: {}", mean, mean_share, bad_routes);
		let secs = tc.delta()?;
		println!("routing: {} s, {}", secs, secs as f64 / iterations as f64);
	}
	Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {
	let args: Vec<_> = std::env::args_os().collect();
	match args.len() {
		2 => {
			run(&args[1].to_str().unwrap())?;
		}
		_ => println!("usage: osm_route GRAPH.CSV",),
	};
	Ok(())
}

#[cfg(test)]
pub mod astar_tests {
	use super::*;
	use osm_route::graph::graph_with_cul_de_sac;

	//  0123456789012345678901
	//                         .
	//4    3---------5---6---8
	//3     \        |    \
	//2  1---2---4   7-----9
	//1              |
	//0             10
	//
	// route 2->10, 2->9, 10->8, 9->2
	#[test]
	fn test_routing() {
		let astar = AstarRouter { graph: &graph_with_cul_de_sac() };
		for (v1, v2, expected_nums) in [
			(2, 10, vec![2, 3, 5, 7, 10]),
			(2, 9,  vec![2, 3, 5, 6, 9]),
			(10, 8, vec![10, 7, 5, 6, 8]),
			(9, 2, vec![9, 6, 5, 3, 2]),
		] {
			println!("\nROUTING {:?} {:?} {:?}", v1, v2, expected_nums);
			let route = astar.shortest_path(&VertexId(v1), &VertexId(v2)).unwrap();
			println!("FW {:?}\n\nBW {:?}\n\n MEET {:?}", route.forward_visited, route.backward_visited, route.meet_vertex);
			assert_eq!(route.vertice_nums().unwrap(), expected_nums);
		}
	}
}

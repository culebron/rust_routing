use indicatif::{ProgressBar, ProgressState, ProgressStyle, ProgressFinish};
use rand::{thread_rng, seq::SliceRandom};
use osm_route::{
	errors::{RoutingError, ok_or_pe},
	isochrone::Isochrone,
	objects::{VertexId, Cost, VertexScore, TimeCheck, VisitedMap},
	graph::{Graph, OnewayPath},
	traits::{GraphPath, Router},
};
use geo::{Point, EuclideanDistance, CoordFloat};
use rayon::prelude::*;
use proj::Proj;
use std::{ collections::{BinaryHeap, HashMap}, error::Error,
};

trait Bearing<T: CoordFloat> {
	fn bearing(&self, other: Point<T>) -> T;
}

impl Bearing<f64> for Point {
	fn bearing(&self, other: Point) -> f64 {
		let dx = self.x() - other.x();
		let dy = self.y() - other.y();
		(dy.atan2(dx).to_degrees() + 360.0) % 360.0
	}
}

const LANDMARKS:usize = 16;

pub struct AltRouter<'a> {
	pub graph: &'a Graph,
	pub center: Point,
	//pub iso: Vec<Isochrone>,
	pub landmark_dist: HashMap<VertexId, Vec<Cost>>
}


impl<'a> AltRouter<'a> {
	pub fn new(graph: &'a Graph) -> Self {
		let vertice = &graph.vertice;
		let mean_lon = vertice.iter().map(|(_, v)| v.geom.x()).sum::<f64>() / vertice.len() as f64;
		let mean_lat = vertice.iter().map(|(_, v)| v.geom.y()).sum::<f64>() / vertice.len() as f64;
		let center = Point::from((mean_lon, mean_lat));

		// iterate vertice, calculate the azimuth from the center, divide by N degrees to get the sector, then keep the farthest in sectors
		let sector_width = 360 as f64 / LANDMARKS as f64;

		let mut farthest = vec![(0usize, 0f64, VertexId(-1)); LANDMARKS];
		for (vid, vtx) in vertice.iter() {
			let dist = vtx.geom.euclidean_distance(&center);
			let bearing = (vtx.geom.bearing(center.clone()) + 360.0) % 360.0;
			let sector = (bearing / sector_width) as usize;
			if dist > farthest[sector].1 {
				farthest[sector] = (sector, dist, *vid);
			}
		}

		// generate isochrones for each node in farthest
		let iso: Vec<Isochrone> = farthest.par_iter().map(|(_, _, n)| Isochrone::try_new(&graph, &n, None).ok()).flatten().collect();

		let mut landmark_dist = HashMap::new();
		for (k, _) in iso[0].distances.iter() {
			let r = iso.iter().map(|i| *i.distances.get(k).unwrap()).collect();
			landmark_dist.insert(*k, r);
		}

		Self { graph, center, landmark_dist }
	}

	pub fn center_4326(&self) -> Result<(f64, f64), Box<dyn Error>> {
		let prj = Proj::new_known_crs("EPSG:3857", "EPSG:4326", None)?;
		Ok(prj.convert(self.center.x_y())?)
	}

	pub fn estimate(&self, v: &VertexId, target: &VertexId) -> Cost {
		let l0 = vec![Cost(0); LANDMARKS];
		let x1 = self.landmark_dist.get(v).unwrap_or(&l0);
		let x2 = self.landmark_dist.get(target).unwrap_or(&l0);
		// we need both d(v) - d(w) and d(w) - d(v), to get the max. Since they're opposite, just take one positive of the two
		let delta: Vec<i64> = x1.iter().zip(x2).map(|(c1, c2)| (c2.0 - c1.0).abs()).collect();
		let cst = delta.iter().max().unwrap_or(&0);
		if (v.0 == 2496884827 || v.0 == 1388196118 || v.0 == 2496884832 || v.0 == 2261287594 || v.0 == 2261287447 || v.0 == 2261287508) && target.0 == 7599909457 {
			println!("estimate v {:?} {:?} target {:?} {:?} delta {:?} max {:?}", v.0, x1, target.0, x2, delta, cst);
		}
		Cost(*cst)

		/*let v1 = self.graph.vertice.get(v).unwrap();
		let v2 = self.graph.vertice.get(target).unwrap();
		let dist = v1.geom.euclidean_distance(&v2.geom) as i64;

		Cost(cst.max(dist))*/
	}
}

impl<'a> Router for AltRouter<'a> {
	type ResultPath = OnewayPath<'a>;
	fn get_graph(&self) -> &Graph {
		self.graph
	}
	fn shortest_path(&self, source: &VertexId, target: &VertexId) -> Result<Self::ResultPath, RoutingError> {
		let mut heap: BinaryHeap<VertexScore> = BinaryHeap::new();
		let mut visited = VisitedMap::new();

		let vs = VertexScore::new(
			source.clone(), source.clone(),
			Cost(0), self.estimate(source, target)
		);
		heap.push(vs);

		let mut visit_number: isize = 0;
		while !heap.is_empty() {
			let mut vs = heap.pop().unwrap();
			vs.visit_number = visit_number;
			visit_number += 1;
			if visited.contains_key(&vs.vid) {
				continue;
			}

			visited.insert(vs.vid, vs.clone());
			if &vs.vid == target {
				return Ok(OnewayPath { visited_: visited, graph: &self.graph, source: *source, target: *target })
			}

			let vtx1 = ok_or_pe(self.graph.get(&vs.vid), "node id not existing")?;

			for e in vtx1.edges.iter() {
				if !visited.contains_key(&e.v2) {
					let vs2 = VertexScore::new(
						e.v2, vs.vid.clone(),
						vs.cost_before + e.weight, self.estimate(&e.v2, target));
					heap.push(vs2);
				}
			}
		}

		return Err(RoutingError::NoRoute { msg: "no route".into() });
	}
}

fn run(path: &str) -> Result<(), Box<dyn Error>> {
	let mut tc = TimeCheck::new();
	let graph = Graph::from_path(path, true)?;
	let alt = AltRouter::new(&graph);
	println!("reading graph: {} s", tc.delta()?);

	let mut rng = thread_rng();
	let vids: Vec<&VertexId> = graph.vertice.keys().collect();

	println!("graph: {:?} vertice", graph.vertice.len());

	let iterations = 1000u64;
	let iterrr = 0..iterations;

	if cfg!(debug_assertions) {
		use osm_route::debug::debug_router;
		debug_router(&alt, "data/alt/")?;
		use wkt::ToWkt;

		let mut cr = csv::Writer::from_path("data/alt/graph.csv")?;
		cr.serialize(("vid", "WKT", "kind", "dists", "max_dist", "mean_dist", "min_dist"))?;
		for (vid, dists) in alt.landmark_dist.iter() {
			let pt = alt.graph.vertice.get(vid).unwrap();
			cr.serialize((
				vid, pt.geom.to_wkt().to_string(), "v",
				dists.iter().map(|i| (i.0 / 100).to_string()).collect::<Vec<String>>().join(";"),
				dists.iter().map(|i| i.0).max().unwrap().to_string(),
				(dists.iter().map(|i| i.0).sum::<i64>() as f64 / dists.len() as f64).to_string(),
				dists.iter().map(|i| i.0).min().unwrap().to_string(),
			))?;
		}
		drop(cr);

	}
	else {
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
			let pth = alt.shortest_path(&vid1, &vid2)?;
			Ok(pth.visited_.len())
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
		_ => println!("usage: alt_route GRAPH.CSV",),
	};
	Ok(())
}

#[cfg(test)]
pub mod alt_tests {
	use osm_route::traits::GraphPath;
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
		let g = graph_with_cul_de_sac();
		let alt = AltRouter::new(&g);
		/*for i in alt.iso.iter() {
			println!("ISO {:?}", i);
		}*/
		for (v1, v2, expected_nums, not_visit) in [
			(2, 10, vec![2, 3, 5, 7, 10], 4),
			(2, 9,  vec![2, 3, 5, 6, 9], 4),
			(10, 8, vec![10, 7, 5, 6, 8], 3),
			(9, 2, vec![9, 6, 5, 3, 2], 7),
		] {
			println!("\nROUTING {:?} {:?} {:?}", v1, v2, expected_nums);
			let id1 = &VertexId(v1);
			let id2 = &VertexId(v2);
			let route = alt.shortest_path(&id1, &id2).unwrap();
			println!("VertexId {:?}\n", route.visited_);
			assert_eq!(route.vertice_nums().unwrap(), expected_nums);
			assert!(!route.visited_.contains_key(&VertexId(not_visit)));
		}
		assert!(false);
	}
}

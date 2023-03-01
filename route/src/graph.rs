use crate::{
	objects::{VertexId, Vertex, RawEdge, Edge, Weight, VisitedMap, Cost},
	traits::GraphPath,
	errors::{ok_or_pe, RoutingError},
};
use csv::Reader;
use geo::{LineString, EuclideanLength, CoordNum, Point};
use num_traits::float::Float;
use proj::Proj;
use std::{ collections::HashMap, error::Error,
	io::Read,
};


// converts geometry to a set projection (Proj instance is required, to reduce num of queries to proj db)
pub fn convert<T>(geom: &LineString<T>, proj: &Proj) -> Result<LineString<T>, Box<dyn Error>>
where T: CoordNum + Float {
	let mut res: Vec<(T, T)> = vec![];
	for c in geom.coords() {
		match proj.convert(c.clone().into()) {
			Ok(v) => res.push(v),
			Err(e) => {
				dbg!(format!("broken coords {:?}", c));
				return Err(Box::new(e));
			}
		}
	}
	Ok(LineString::<T>::from(res))
}

#[derive(Debug)]
pub struct Graph {
	pub vertice: HashMap<VertexId, Vertex>,
}

impl Graph {
	pub fn new() -> Self { Self { vertice: HashMap::new() } }

	pub fn add_edge(&mut self, edge: Edge, bidirectional: bool) {
		let geom = edge.geom.clone().into_points()[0];

		let vid = edge.v1;
		let v = self.vertice.entry(vid).or_insert_with(|| Vertex {
				id: vid, edges: vec![], geom });
			v.edges.push(edge.clone());

		if bidirectional {
			let mut ee = edge;
			(ee.v1, ee.v2) = (ee.v2, ee.v1);
			ee.geom.0.reverse();
			self.add_edge(ee, false);
		}
	}

	pub fn get_edge(&self, a: &VertexId, b: &VertexId) -> Option<&Edge> {
		match self.vertice.get(a) {
			Some(va) => {
				for e in va.edges.iter() {
					if e.v2 == *b {
						return Some(e)
					}
				}
				None
			},
			None => None
		}
	}

	pub fn from_path(path: &str, project: bool) -> Result<Self, Box<dyn Error>> {
		Self::parse_csv(Reader::from_path(path)?, project)
	}

	pub fn from_reader<R: Read>(buf: R, project: bool) -> Result<Self, Box<dyn Error>> {
		Self::parse_csv(Reader::from_reader(buf), project)
	}

	pub fn parse_csv<R: Read>(mut csvr: Reader<R>, project: bool) -> Result<Self, Box<dyn Error>> {
		let mut g = Self::new();
		let converter = project.then(|| Proj::new_known_crs("EPSG:4326", "EPSG:3857", None).unwrap());

		for e in csvr.deserialize() {
			let e: RawEdge = e?;
			let geom = if let Some(ref c) = converter { convert(&e.WKT, &c)? } else { e.WKT };
			let weight = Weight(geom.euclidean_length().round() as u64);

			g.add_edge(Edge { v1: e.node1, v2: e.node2, geom: geom.clone(), weight }, true); // TODO: decide on bidirectional
		};
		Ok(g)
	}

	pub fn get_mut(&mut self, k: &VertexId) -> Option<&mut Vertex> { self.vertice.get_mut(k) }
	pub fn get(&self, k: &VertexId) -> Option<&Vertex> { self.vertice.get(k) }
	pub fn insert(&mut self, k: VertexId, v: Vertex) -> Option<Vertex> { self.vertice.insert(k, v) }
	pub fn get_num_vertice(&self) -> usize { self.vertice.len() }
}

#[derive(Debug)]
pub struct OnewayPath<'a> {
	pub visited_: VisitedMap,
	pub source: VertexId,
	pub target: VertexId,
	pub graph: &'a Graph,
}

impl<'a> GraphPath for OnewayPath<'a> {
	fn cost(&self) -> Result<Cost, RoutingError> {
		let vs = ok_or_pe(self.visited_.get(&self.target), "vertex not in visited although it must be")?;
		Ok(vs.cost_before)
	}
	fn edges(&self) -> Result<Vec<Edge>, RoutingError> {
		let vertice = self.vertice()?;
		let mut edges = vec![];
		for i in 1..vertice.len() {
			edges.push(ok_or_pe(self.graph.get_edge(&vertice[i - 1], &vertice[i]), "vertex not in the graph")?.clone());
		}
		Ok(edges)
	}
	fn vertice(&self) -> Result<Vec<VertexId>, RoutingError> {
		let mut vs = ok_or_pe(self.visited_.get(&self.target), &format!("vertex {} not in visited although it must be", self.target.0))?;
		let mut vt = vec![vs.vid.clone()];
		while vs.vid != self.source {
			vs = ok_or_pe(self.visited_.get(&vs.from), &format!("vertex {} not in visited although it must be", vs.from.0))?;
			vt.push(vs.vid.clone());
		}
		vt.reverse();
		Ok(vt)
	}
	fn vertice_nums(&self) -> Result<Vec<i64>, RoutingError> {
		Ok(self.vertice()?.iter().map(|v| v.0).collect())
	}
	fn edge_nums(&self) -> Result<Vec<(i64, i64)>, RoutingError> {
		Ok(self.edges()?.iter().map(|e| (e.v1.0, e.v2.0)).collect())
	}
	fn vertice_geoms(&self) -> Result<Vec<Point>, RoutingError> {
		let mut pts = vec![];
		for v in self.vertice()?.iter() {
			pts.push(ok_or_pe(self.graph.get(v), "vertex not in graph")?.geom);
		}
		pts.reverse();
		Ok(pts)
	}
	fn visited(&self) -> Vec<&VisitedMap> {
		vec![&self.visited_]
	}
}


#[derive(Debug)]
pub struct BidirPath<'a> {
	pub forward_visited: VisitedMap,
	pub backward_visited: VisitedMap,
	pub meet_vertex: VertexId,
	pub graph: &'a Graph,
}

impl<'a> BidirPath<'a> {
	fn _collect_vertice(&self, scores: &VisitedMap) -> Result<Vec<VertexId>, RoutingError> {
		let mut result = vec![self.meet_vertex.clone()];
		let mut current_id = self.meet_vertex;

		loop {
			let current_vs = ok_or_pe(scores.get(&current_id), &format!("meet vertex {} is not in visited vertice {:?}", current_id.0, scores.keys().map(|v| v.0).collect::<Vec<i64>>()))?;
			if current_vs.vid == current_vs.from { break; };
			current_id = current_vs.from;
			result.push(current_vs.from.clone());
		}
		Ok(result)
	}
}

impl<'a> GraphPath for BidirPath<'a> {
	fn cost(&self) -> Result<Cost, RoutingError> {
		let f = ok_or_pe(self.forward_visited.get(&self.meet_vertex), "vertex not in forward map although it must be")?;
		let b = ok_or_pe(self.backward_visited.get(&self.meet_vertex), "vertex not in backward map although it must be")?;
		Ok(f.cost_before + b.cost_before)
	}
	fn edges(&self) -> Result<Vec<Edge>, RoutingError> {
		let vertice = self.vertice()?;
		let mut edges = vec![];
		for i in 1..vertice.len() {
			edges.push(ok_or_pe(self.graph.get_edge(&vertice[i - 1], &vertice[i]), "vertex not in the graph")?.clone());
		}
		Ok(edges)
	}
	fn vertice(&self) -> Result<Vec<VertexId>, RoutingError> {
		let mut a = self._collect_vertice(&self.forward_visited)?;
		a.reverse();
		let mut b = self._collect_vertice(&self.backward_visited)?;
		b.remove(0);
		Ok(a.into_iter().chain(b.into_iter()).collect())
	}
	fn vertice_nums(&self) -> Result<Vec<i64>, RoutingError> {
		Ok(self.vertice()?.iter().map(|v| v.0).collect())
	}
	fn vertice_geoms(&self) -> Result<Vec<Point>, RoutingError> {
		let mut pts = vec![];
		for v in self.vertice()?.iter() {
			pts.push(ok_or_pe(self.graph.get(v), "vertex not in graph")?.geom);
		}
		pts.reverse();
		Ok(pts)
	}
	fn edge_nums(&self) -> Result<Vec<(i64, i64)>, RoutingError> {
		Ok(self.edges()?.iter().map(|e| (e.v1.0, e.v2.0)).collect())
	}
	fn visited(&self) -> Vec<&VisitedMap> {
		vec![&self.forward_visited, &self.backward_visited]
	}
}

// imported from astar.rs, hence should be outside #[cfg(test)]
pub fn make_graph() -> Graph {
	let mut g = Graph::new();
	let geom = LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]);

	let edges: Vec<(i64, i64)> = vec![
		(1, 2), (2, 3), (2, 4), (3, 5),
		(4, 6), (6, 7), (6, 8), (7, 9),
		(8, 9), (8, 10), (9, 11),
	];

	for (vid1, vid2) in edges.iter() {
		let (v1, v2) = (VertexId(*vid1), VertexId(*vid2));
		g.add_edge(Edge { v1, v2, weight: Weight(1), geom: geom.clone() }, true)
	}
	g
}

//   0123456789012345678901
//                         .
// 4    3---------5---6---8
// 3     \        |    \
// 2  1---2---4   7-----9
// 1              |
// 0             10
//
// route 2->10, 2->9, 10->8, 9->2
pub fn graph_with_cul_de_sac() -> Graph {
	let mut g = Graph::new();
	let vrt: HashMap<i64, (f64, f64)> = vec![
		(1, (2.0, 1.0)), (2, (2.0, 5.0)), (3, (4.0, 3.0)), (4, (2.0, 9.0)), (5, (4.0, 13.0)),
		(6, (4.0, 17.0)), (7, (2.0, 13.0)), (8, (4.0, 21.0)), (9, (2.0, 21.0)), (10,(0.0, 13.0))
	].into_iter().map(|(i, (c2, c1))| {
		(i, (c1*10.0, c2*20.0))
	}).collect();
	vec![
		(3, 5), (5, 6), (6, 8), (3, 2), (5, 7),
		(6, 9), (1, 2), (2, 4), (7, 9), (7, 10),
	].into_iter().for_each(|(n1, n2)| {
		let v1 = VertexId(n1);
		let v2 = VertexId(n2);
		let c1 = vrt.get(&n1).unwrap().clone();
		let c2 = vrt.get(&n2).unwrap().clone();
		let geom = LineString::from(vec![c1, c2]);

		let e = Edge { v1, v2, geom: geom.clone(), weight: Weight(geom.euclidean_length() as u64) };
		g.add_edge(e, true);
	});
	g
}

#[cfg(test)]
pub mod graph_tests {
	use super::*;
	use crate::objects::VertexScore;

	#[test]
	fn from_edges_works() {
		//
		//	1---2---4---6---8---10
		//	    |   |   |   |
		// 	    3---5   7---9---11
		let g = make_graph();

		// test edges from vertex 1. It should be from 1 -> 2.
		let v1 = g.vertice.get(&VertexId(1)).unwrap();
		assert_eq!(v1.edges.len(), 1);
		let e = v1.edges[0].clone();
		assert_eq!(e.v1, VertexId(1));
		assert_eq!(e.v2, VertexId(2));

		// test edges from vertex 6 (6->4, 6->7, 6->8)
		let v6 = g.vertice.get(&VertexId(6)).unwrap();
		assert_eq!(v6.edges.len(), 3);
		dbg!(format!("EDGES FROM V6 {:?}", v6.edges));
		let mut others: Vec<i64> = v6.edges.iter().map(|e| e.v2.0).collect::<Vec<_>>();
		others.sort();
		assert_eq!(others, vec![4, 7, 8]);

		dbg!(format!("{:?}", g.vertice.len()));
		assert!(g.vertice.len() == 11);
	}

	#[test]
	fn from_edge_coords_correct() {
		// initially, coordinates were confused in add_edge.
		let mut g = Graph::new();
		g.add_edge(Edge { v1: VertexId(5), v2: VertexId(10), geom: LineString::from(vec![(12., 14.0), (39., 45.0), (48., 55.0)]), weight: Weight(100) }, true);
		let v1 = g.get(&VertexId(5)).unwrap();
		let v2 = g.get(&VertexId(10)).unwrap();
		assert_eq!((v1.geom.x(), v1.geom.y()), (12.0, 14.0));
		assert_eq!((v2.geom.x(), v2.geom.y()), (48.0, 55.0));
		assert_eq!(v1.edges.len(), 1);
		assert_eq!(v2.edges.len(), 1);
	}

	fn to_hm(m: &[(i64, i64, i64)]) -> VisitedMap {
		m.iter()
		.map(|(f, v, c)| (VertexId(*f), VertexId(*v), Cost(*c)))
		.map(|(from, vid, c)| (vid, VertexScore::new(vid, from, c, Cost(0)))).collect()
	}

	fn make_bp(meet_vertex: VertexId, graph: &Graph) -> BidirPath {
		let forward_visited = to_hm(&[
			(1, 1, 0), (1, 2, 1), (2, 3, 2),
			(2, 4, 2), (4, 5, 3), (4, 6, 3)]);
		let backward_visited = to_hm(&[
			(11, 11, 0), (11, 9, 1), (9, 8, 2),
			(9, 7, 2), (8, 10, 3), (7, 6, 3)]);

		BidirPath { forward_visited, backward_visited, meet_vertex, graph }
	}

	#[test]
	fn test_bidir_works() {
		let g = make_graph();

		//  1---2---4---6---8---10
		//      |   |   |   |
		//      3---5   7---9---11

		let bp = make_bp(VertexId(6), &g);
		assert_eq!(bp.cost().unwrap(), Cost(6));

		// check vertice in the path
		assert_eq!(bp.vertice_nums().unwrap(), vec![1, 2, 4, 6, 7, 9, 11]);

		// check the ids in the edges
		let edg: Vec<(i64, i64)> = bp.edge_nums().unwrap();
		assert_eq!(edg, vec![(1, 2), (2, 4), (4, 6), (6, 7), (7, 9), (9, 11)]);
	}

	#[test]
	fn test_bidir_fails() {
		let g = make_graph();
		let bp2 = make_bp(VertexId(123), &g);
		assert!(bp2.cost().is_err());
		assert!(bp2.vertice().is_err());
		assert!(bp2.edges().is_err());
	}
}

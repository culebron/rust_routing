use rand::{thread_rng, seq::SliceRandom};
use geo::LineString;
use serde::Serialize;
use wkt::ToWkt;
use csv;
use crate::{
	traits::{Router, GraphPath},
	objects::{VertexId, Cost}
};
use std::error::Error;

#[derive(Serialize)]
#[allow(non_snake_case)]
struct VisitedVertex {
	vid: VertexId,
	from: VertexId,
	WKT: String,
	cost_before: Cost,
	cost_remain: Cost,
	visit_number: isize,
}

pub fn debug_router<R: Router>(router: &R, path_prefix: &str) -> Result<(), Box<dyn Error>> {
	println!("debug!");
	let mut rng = thread_rng();
	let vids: Vec<&VertexId> = router.get_graph().vertice.keys().collect();
	let mut pairs: Vec<(usize, &VertexId, &VertexId)> = (0..1).map(|i| {
		(i, *vids[..].choose(&mut rng).unwrap(), *vids[..].choose(&mut rng).unwrap())
	}).collect();
	pairs.push((5, &VertexId(3184454140), &VertexId(7599909457)));

	for (i, v1, v2) in pairs.into_iter() {
		let mut cr = csv::Writer::from_path(format!("{}route_{}.csv", path_prefix, i))?;
		let vtx1 = router.get_graph().get(&v1).unwrap();
		let vtx2 = router.get_graph().get(&v2).unwrap();
		let ls: LineString = vec![vtx1.geom, vtx2.geom].into_iter().collect();
		let wkt = ls.to_wkt().to_string();
		let o = VisitedVertex { WKT: wkt, vid: *v2, from: *v1, cost_before: Cost(0), cost_remain: Cost(0), visit_number: 0 };
		cr.serialize(o)?;
		drop(cr);

		let mut cr = csv::Writer::from_path(format!("{}visited_{}.csv", path_prefix, i))?;
		let pth = router.shortest_path(&v1, &v2)?;
		for hm in pth.visited().into_iter() {
			println!("visited {:?}", hm.len());
			for (_k, vs) in hm.iter() {
				let vtx1 = router.get_graph().get(&vs.from).unwrap();
				let vtx2 = router.get_graph().get(&vs.vid).unwrap();
				let ls: LineString = vec![vtx1.geom, vtx2.geom].into_iter().collect();
				let wkt = ls.to_wkt().to_string();
				let o = VisitedVertex { WKT: wkt, cost_before: vs.cost_before, cost_remain: vs.cost_remain, vid: vs.vid, from: vs.from, visit_number: vs.visit_number };
				cr.serialize(o)?;
			}
		}
		drop(cr);
	}
	Ok(())
}

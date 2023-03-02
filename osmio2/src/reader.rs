use crossbeam_channel::{Receiver, Sender, bounded, SendError, IntoIter as CbIntoIter};
use bzip2::read::BzDecoder;
use crate::{
	errors::ReadError,
	objects::{
		Member, OsmObj, ObjType, Tags,
		Node, Way, Relation,
		OsmElementAttrs, ParsedAttrs}
};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressState, ProgressStyle, ProgressFinish};
use quick_xml::{
	Reader,
	events::{Event, BytesStart},
};
use std::{
	error::Error,
	fs::File,
	io::{Read, BufReader},
	path::Path,
	sync::Arc,
	str::from_utf8,
	thread::spawn,
};

pub struct OsmXmlReader {
	pub rd: Reader<BufReader<Box<dyn Read + Send>>>,
	pub elt: Option<OsmObj>,
	pub skip_nodes: bool,
	pub skip_ways: bool,
	pub skip_relations: bool,
	pub curr_elt: Option<ObjType>,
}

type OkOrBox = Result<(), Box<dyn Error>>;
pub type OsmXmlItem = Result<OsmObj, ReadError>;
impl OsmXmlReader {
	pub fn new(rd: BufReader<Box<dyn Read + Send>>) -> OsmXmlReader {
		Self {rd: Reader::from_reader(rd), elt: None, skip_nodes: false, skip_ways: false, skip_relations: false, curr_elt: None }
	}

	pub fn from_path(path: &str) -> Result<OsmXmlReader, Box<dyn Error>> {
		// a wrapper for flat/gzipped/bzipped files
		let fp = File::open(path)?;
		let size = fp.metadata().unwrap().len();
		let pbr = ProgressBar::new(size)
			.with_style(ProgressStyle::with_template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
			.unwrap()
			.with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
			.progress_chars("#>-"))
			.with_finish(ProgressFinish::Abandon)
			.wrap_read(fp);

		let rd = if path.ends_with(".osm.gz") {
			Box::new(GzDecoder::new(pbr)) as Box<dyn Read + Send>
		} else if path.ends_with(".osm.bz2") {
			Box::new(BzDecoder::new(pbr)) as Box<dyn Read + Send>
		} else if path.ends_with(".osm") {
			Box::new(pbr) as Box<dyn Read + Send>
		} else {
			return Err("file is not .osm format".into());
		};

		Ok(Self::new(BufReader::new(rd)))
	}

	pub fn in_background(self) -> CbIntoIter<OsmXmlItem> {
		let (snd, rec):(Sender<OsmXmlItem>, Receiver<OsmXmlItem>) = bounded(5);
		spawn(move || -> Result<(), SendError<OsmXmlItem>> {
			// println!("running in background!");
			for obj in self.into_iter() {
				snd.send(obj)?
			}
			drop(snd);
			Ok(())
		});
		rec.into_iter()
	}

	fn _attrs_hashmap(&mut self, elt: &BytesStart) -> Result<ParsedAttrs, ReadError> {
		let mut hm = ParsedAttrs::new();
		for e in elt.attributes() {
			let e = e?;
			let k = from_utf8(e.key)?;
			hm.insert(
				Arc::from(k),
				Arc::from(e.unescape_and_decode_value(&self.rd)?.as_str())
			);
		}
		Ok(hm)
	}

	fn _process_elements(&mut self, elts: Vec<BytesStart>) -> Result<Option<OsmObj>, ReadError> {
		let elt = match elts.first() {
			None => return Ok(None),
			Some(elt) => elt
		};

		let attrs_hashmap = self._attrs_hashmap(elt)?;
		let tags = Tags::new();
		let osm_attrs = OsmElementAttrs::try_from(&attrs_hashmap)?;
		let mut res = match elt.name() {
			b"node" => {
				let lon:f32 = attrs_hashmap.get("lon").ok_or_else(|| ReadError { msg: "node has no longitude".to_string()})?.parse()?;
				let lat:f32 = attrs_hashmap.get("lat").ok_or_else(|| ReadError{ msg: "node has no latitude".to_string()})?.parse()?;
				OsmObj::Node(Node { attrs: osm_attrs, lon: lon, lat: lat, tags: tags })
			},
			b"way" => {
				OsmObj::Way(Way { attrs: osm_attrs, tags: tags, nodes: Vec::new() })
			},
			b"relation" => {
				OsmObj::Relation(Relation { attrs: osm_attrs, tags: tags, members: vec![] })
			}
			x => { panic!("wrong tag on pos 1 in tags vector: {:?}", x) },
		};

		for elt in &elts[1..elts.len()] {
			match elt.name() {
				b"tag" => {
					let hm = self._attrs_hashmap(elt)?;
					let k = hm.get("k");
					let v = hm.get("v");
					if let (Some(k1), Some(v1)) = (k, v) {
						res.tags_insert(k1.clone(), v1.clone());
					}
				},
				b"nd" => {
					if let OsmObj::Way(ref mut w) = res {
						let hm = self._attrs_hashmap(elt)?;
						hm.get("ref").map(|nd| nd.parse::<i64>()).transpose()?.map(|nd| w.nodes.push(nd));
					}
				},
				b"member" => {
					if let OsmObj::Relation(ref mut r) = res {
						let hm = self._attrs_hashmap(elt)?;
						let mtype:ObjType = hm.get("type").ok_or_else(|| ReadError { msg: "member element has no 'type' attribute".to_string() })?.clone().try_into()?;
						let mref:i64 = hm.get("ref").ok_or_else(|| ReadError { msg: "member element has no 'ref' attribute".to_string() })?.parse()?;
						let mrole = hm.get("role").ok_or_else(|| ReadError { msg: "member element has no 'role' attribute".to_string() })?.clone();
						r.members.push(Member { mtype: mtype, mref: mref, mrole: mrole })
					}
				}
				_ => {}
			}
		}

		Ok(Some(res))
	}

	pub fn _next(&mut self) -> Result<Option<OsmObj>, ReadError> {
		let mut buf = Vec::new();
		let mut elements: Vec<BytesStart> = Vec::new();

		let mut obj_started = false;
		let mut do_skip: bool = false;

		loop {
			let e1 = self.rd.read_event(&mut buf);
			match (obj_started, &e1) {
				(_, Err(e)) => { panic!("Error at position {}: {:?}", self.rd.buffer_position(), e) },
				(false, Ok(Event::Start(ref e2)) | Ok(Event::Empty(ref e2))) => {
					let nm = e2.name();
					if matches!(nm, b"nd" | b"tag" | b"member") {
						panic!("nd/tag/member outside of node/way/relation")
					}

					do_skip = match nm {
						b"node" => self.skip_nodes,
						b"way" => self.skip_ways,
						b"relation" => self.skip_relations,
						_ => false
					};

					if matches!(nm, b"node" | b"way" | b"relation") {
						obj_started = true;
						if !do_skip {
							elements.push(e2.to_owned());
						}
						if matches!(e1, Ok(Event::Empty(_))) {
							obj_started = false;
							if !do_skip {
								return self._process_elements(elements);
							}
						}
					}
				},
				(true, Ok(Event::Start(ref e2)) | Ok(Event::Empty(ref e2))) => {
					let nm = e2.name();
					if matches!(nm, b"node" | b"way" | b"relation") {
						panic!("node/way/relation inside another")
					};
					if !do_skip { elements.push(e2.to_owned()); }
				},
				(true, Ok(Event::End(ref e2))) => {
					if do_skip {
						do_skip = false;
						obj_started = false;
						continue;
					}
					if matches!(e2.name(), b"node" | b"way" | b"relation") {
						return self._process_elements(elements);
					}
				},
				(_, Ok(Event::Eof)) => {
					return Ok(None)
				},
				_ => {}
			}

			// if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
			buf.clear();
		}
	}

	pub fn map_nodes<F>(&mut self, mut cb: F) -> OkOrBox
	where F: FnMut(Node) -> OkOrBox {
		self.skip_ways = true;
		self.skip_relations = true;
		for res in self.into_iter() {
			if let OsmObj::Node(n) = res? { cb(n)? }
		}
		Ok(())
	}

	pub fn map_ways<F>(&mut self, mut cb: F) -> OkOrBox
	where F: FnMut(Way) -> OkOrBox {
		self.skip_relations = true;
		self.skip_nodes = true;
		for res in self.into_iter() {
			if let OsmObj::Way(w) = res? { cb(w)? }
		}
		Ok(())
	}

	// pub fn map_all
}

impl Iterator for OsmXmlReader {
	type Item = Result<OsmObj, ReadError>;
	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		// method _next() was implemented here, because
		// returning Result<Option<T>, E> is much simpler in terms of syntax, than Option<Result<T, E>>
		// (the latter way, you can't use `?` at all).
		self._next().transpose()
		// sorry, methods _next, _parse_items, etc. can't be placed here in `impl Iterator` block
	}
}


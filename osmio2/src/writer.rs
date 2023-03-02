use crossbeam_channel::{Receiver, Sender, bounded, SendError};
use flate2::{Compression as gzCompression, write::GzEncoder};
use bzip2::{Compression as bzCompression, write::BzEncoder};
use quick_xml::{
	events::{Event, BytesDecl, BytesStart, BytesEnd},
	Writer,
};

use std::{
	error::Error,
	fs::File,
	io::Write,
	sync::Arc,
	thread::{JoinHandle, spawn, Result as TResult},
};

use crate::{
	objects::OsmObj,
	errors::WriteError
};

pub struct OsmXmlWriter {
	pub wr: Writer<Box<dyn Write + Send>>
}

impl OsmXmlWriter {
	pub fn new(wr: Box<dyn Write + Send>) -> Result<OsmXmlWriter, Box<dyn Error>> {
		let mut wr1 = Writer::new_with_indent(wr, 9, 1);  // char 9 = \t (ASCII tab)
		wr1.write_event(Event::Decl(
			BytesDecl::from_start(
				BytesStart::borrowed(b"xml version='1.0' encoding='UTF-8'", 3))))?;
		let mut open_tag = BytesStart::owned("osm", "osm".len());
		open_tag.push_attribute(("version", "0.6"));
		open_tag.push_attribute(("generator", "cosmos"));
		wr1.write_event(Event::Start(open_tag))?;

		Ok(OsmXmlWriter { wr: wr1 })
	}

	pub fn from_path(path: &str) -> Result<OsmXmlWriter, Box<dyn Error>> {
		let fp = File::create(path)?;
		let wr = if path.ends_with(".osm") {
			Box::new(fp) as Box<dyn Write + Send>
		} else if path.ends_with(".osm.gz") {
			Box::new(GzEncoder::new(fp, gzCompression::new(5))) as Box<dyn Write + Send>
		} else if path.ends_with(".osm.bz2") {
			Box::new(BzEncoder::new(fp, bzCompression::Default)) as Box<dyn Write + Send>
		} else {
			return Err("file is not .osm format".into())
		};

		Self::new(wr)
	}

	pub fn write(&mut self, osmobj: &OsmObj) -> Result<(), WriteError> {
		let (tagname, tagnamelen, tags, nodes, members) = match osmobj {
			OsmObj::Node(ref n) => { ("node", 4, n.tags.clone(), vec![], vec![]) },
			OsmObj::Way(ref w) => { ("way", 3, w.tags.clone(), w.nodes.clone(), vec![]) },
			OsmObj::Relation(ref r) => { ("relation", 8, r.tags.clone(), vec![], r.members.clone()) },
		};

		let mut start_elt = BytesStart::owned(tagname, tagnamelen);
		let has_inner = tags.len() + nodes.len() + members.len() > 0;
		match osmobj {
			OsmObj::Node(node) => {
				node.attrs.push_to(&mut start_elt);
				start_elt.push_attribute(("lon", &node.lon.to_string() as &str));
				start_elt.push_attribute(("lat", &node.lat.to_string() as &str));
			},
			OsmObj::Way(way) => {
				way.attrs.push_to(&mut start_elt);
			},
			OsmObj::Relation(rel) => {
				rel.attrs.push_to(&mut start_elt);

			}
		};

		if !has_inner {
			self.wr.write_event(Event::Empty(start_elt))?;
			return Ok(())
		}

		self.wr.write_event(Event::Start(start_elt))?;
		for n in members {
			let mut nelt = BytesStart::owned("member", 6);
			nelt.push_attribute(("type", n.mtype.into()));
			nelt.push_attribute(("ref", &n.mref.to_string() as &str));
			nelt.push_attribute(("role", &*n.mrole));
			self.wr.write_event(Event::Empty(nelt))?;
		}
		for (k, v) in tags {
			let mut nelt = BytesStart::owned("tag", 3);
			nelt.push_attribute(("k", &*k));
			nelt.push_attribute(("v", &*v));
			self.wr.write_event(Event::Empty(nelt))?;
		}
		for n in nodes {
			let mut nelt = BytesStart::owned("nd", 2);
			nelt.push_attribute(("ref", &n.to_string() as &str));
			self.wr.write_event(Event::Empty(nelt))?;
		}

		self.wr.write_event(Event::End(BytesEnd::owned(tagname.as_bytes().to_vec())))?;
		Ok(())
	}

	pub fn close(&mut self) -> Result<(), WriteError> {
		self.wr.write_event(Event::End(BytesEnd::owned(b"osm".to_vec())))?;
		Ok(())
	}
}


pub struct BgWriter {
	handle: Option<JoinHandle<Result<(), WriteError>>>,
	sender: Option<Sender<Arc<OsmObj>>>,
}

impl BgWriter {
	pub fn new(mut wr: OsmXmlWriter) -> BgWriter {
		let (sender, receiver):(Sender<Arc<OsmObj>>, Receiver<Arc<OsmObj>>) = bounded(5);
		let writer_thread = spawn(move || -> Result<(), WriteError> {
			while let Ok(osmobj) = receiver.recv() {
				wr.write(&*osmobj)?;
			}
			wr.close()?;
			drop(receiver);
			Ok(())
		});
		BgWriter { handle: Some(writer_thread), sender: Some(sender) }
	}

	pub fn write(&self, item: OsmObj) -> Result<(), SendError<Arc<OsmObj>>> {
		if let Some(ref sender) = self.sender.as_ref() {
			sender.send(Arc::new(item))
		} else { Ok(()) }
	}

	pub fn close(&mut self) -> TResult<()> {
		if let Some(sender) = self.sender.take() { drop(sender); }
		if let Some(handle) = self.handle.take() { handle.join()?; }
		Ok(())
	}
}

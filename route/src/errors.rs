use std::{error::Error, fmt};

#[derive(Debug)]
pub enum RoutingError {
	NoRoute { msg: String },
	Programming { msg: String }
}
impl Error for RoutingError {}
impl fmt::Display for RoutingError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = match self { Self::NoRoute { msg } => msg, Self::Programming { msg } => msg };
		write!(f, "Write error: {}", &msg)
	}
}

pub fn ok_or_nr<T>(data: Option<T>, msg: &str) -> Result<T, RoutingError> {
	data.ok_or(RoutingError::NoRoute { msg: msg.into() })
}

pub fn ok_or_pe<T>(data: Option<T>, msg: &str) -> Result<T, RoutingError> {
	data.ok_or(RoutingError::Programming { msg: msg.into() })
}


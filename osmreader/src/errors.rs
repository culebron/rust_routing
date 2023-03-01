use quick_xml::{
	events::attributes::AttrError,
	Error as QError
};

use std::{
	error::Error,
	fmt,
	num::{ParseIntError, ParseFloatError},
	str::{ParseBoolError, Utf8Error},
	string::FromUtf8Error,
};

#[derive(Debug, Clone)]
pub struct ReadError {
	pub msg: String
}

impl Error for ReadError {}

impl fmt::Display for ReadError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Parse error: {}", &self.msg)
	}
}

impl From<AttrError> for ReadError {
	fn from(e: AttrError) -> Self {
		Self { msg: format!("Attribute error {:?}", e)}
	}
}

impl From<QError> for ReadError {
	fn from(e: QError) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

impl From<ParseIntError> for ReadError {
	fn from(e: ParseIntError) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

impl From<ParseFloatError> for ReadError {
	fn from(e: ParseFloatError) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

impl From<ParseBoolError> for ReadError {
	fn from(e: ParseBoolError) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

impl From<FromUtf8Error> for ReadError {
	fn from(e: FromUtf8Error) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

impl From<Utf8Error> for ReadError {
	fn from(e: Utf8Error) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

#[derive(Debug, Clone)]
pub struct WriteError {
	pub msg: String
}

impl Error for WriteError {}

impl fmt::Display for WriteError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Write error: {}", &self.msg)
	}
}

impl From<QError> for WriteError {
	fn from(e: QError) -> Self {
		Self { msg: format!("Parsing error {:?}", e)}
	}
}

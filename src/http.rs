
use std::io::Write;

use anyhow::Result;


#[derive(Clone, Copy, Debug)]
pub enum HttpMethod {
	GET, POST, ERASE,
}

impl HttpMethod {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::GET => "GET",
			Self::POST => "POST",
			Self::ERASE => "ERASE",
		}
	}
}

#[derive(Clone, Copy)]
pub enum HttpHeader<'a> {
	ContentType(ContentType),
	TransferEncoding(TransferEncoding),
	ContentDisposition(ContentDisposition<'a>),
	ContentLength(usize),
}

impl<'a> HttpHeader<'a> {
	pub fn from_str_pair(key: &str, value: &'a str) -> Option<Self> {
		match key {
			"Content-Type" => {
				if let Some(ctype) = ContentType::from_str(value) {
					return Some(HttpHeader::ContentType(ctype));
				}else { return None; }
			},
			"Transfer-Encoding" => {
				if let Some(encoding) = TransferEncoding::from_str(value) {
					return Some(HttpHeader::TransferEncoding(encoding));
				}else { return None; }
			},
			"Content-Length" => {
				// if let Ok(value.parse::<usize>())
				if let Ok(len) = value.parse::<usize>() {
					return Some(HttpHeader::ContentLength(len));
				} else { return None; }
			},
			"Content-Disposition" => { todo!() },
			_ => None,
		}
	}
}

impl ReadInto for HttpHeader<'_> {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		let mut write_size = 0;
		match self {
			HttpHeader::ContentType(ctype) => {
				write_size += destination.write(b"Content-Type: ")?;
				write_size += destination.write(ctype.as_str().as_bytes())?;
			},
			HttpHeader::TransferEncoding(enc) => {
				write_size += destination.write(b"Transfer-Encoding: ")?;
				write_size += destination.write(enc.as_str().as_bytes())?;
			},
			HttpHeader::ContentDisposition(disp) => write_size += disp.read_into(destination)?,
			HttpHeader::ContentLength(len) => {
				write_size += destination.write(b"Content-Length: ")?;
				write_size += len.read_into(destination)?;
			}
		};
		write_size += destination.write(b"\r\n")?;
		return Ok(write_size);
	}
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
pub enum ContentType {
	text_html,
	text_plain,
	image_x_icon,
	audio_flac,
	application_json
}

impl ContentType {
	pub fn from_str(ctype: &str) -> Option<ContentType> {
		match ctype {
			"text/html" => Some(ContentType::text_html),
			"text/plain" => Some(ContentType::text_plain),
			"image/x-icon" => Some(ContentType::image_x_icon),
			"audio/flac" => Some(ContentType::audio_flac),
			_ => None
		}
	}

	pub fn as_str(self) -> &'static str {
		match self {
			Self::text_html => "text/html",
			Self::text_plain => "text/plain",
			Self::image_x_icon => "image/x-icon",
			Self::audio_flac => "audio/flac",
			Self::application_json => "application/json",
		}
	}
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
pub enum TransferEncoding {
	_7bit,
	_8bit,
	binary,
	quoted_printable,
	base64
}

impl TransferEncoding {
	pub fn from_str(source: &str) -> Option<Self> {
		match source {
			"7bit" => Some(Self::_7bit),
			"8bit" => Some(Self::_8bit),
			"binary" => Some(Self::binary),
			"quoted_printable" => Some(Self::quoted_printable),
			"base64" => Some(Self::base64),
			_ => None
		}
	}

	pub fn as_str(self) -> &'static str {
		match self {
			Self::_7bit => "7bit",
			Self::_8bit => "8bit",
			Self::binary => "binary",
			Self::quoted_printable => "quoted_printable",
			Self::base64 => "base64",
		}
	}
}

#[derive(Clone, Copy)]
pub enum ContentDisposition<'a> {
	Inline,
	Attachment(Option<&'a str>)
}

impl ReadInto for ContentDisposition<'_> {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		let mut write_size = destination.write(b"Content-Disposition: ")?;
		match self {
			ContentDisposition::Inline => write_size += destination.write(b"inline")?,
			ContentDisposition::Attachment(None) => write_size += destination.write(b"attachment")?,
			ContentDisposition::Attachment(Some(fname)) => {
				write_size += destination.write(b"attachment; filename=")?;
				write_size += destination.write(fname.as_bytes())?;
				write_size += destination.write(b"; filename*=")?;
				write_size += destination.write(fname.as_bytes())?;
			}
		}
		return Ok(write_size);
	}
}


pub struct StreamBuffer<'a> {
	buffer: &'a mut [u8],
	filled: usize,
	pub stream: &'a mut dyn Write
}

impl<'a> StreamBuffer<'a> {
	pub fn new(buffer: &'a mut [u8], stream: &'a mut dyn Write) -> Self {
		return Self{ buffer, stream, filled: 0 };
	}
	pub fn push_http_response_primary_header<
		A: AsRef<[u8]>, B: AsRef<[u8]>
	>(&mut self, protocol_version: A, code: usize, status: B) -> Result<()> {
		self.write(protocol_version.as_ref())?;
		write!(self, " {code} ")?;
		self.write(status.as_ref())?;
		self.write(b"\r\n")?;
		return Ok(());
	}
	pub fn push_http_request_primary_header(
		&mut self, method: HttpMethod, route: &str, protocol_version: &str
	) -> Result<()> {
		write!(self, "{} {} {}", method.as_str(), route, protocol_version)?;
		return Ok(());
	}
	pub fn push_http_content_type(&mut self, ctype: ContentType) -> Result<()> {
		self.write(b"Content-Type: ")?;
		self.write(ctype.as_str().as_bytes())?;
		self.write(b"\r\n")?;
		return Ok(());
	}
	pub fn push_http_transfer_encoding(&mut self, encoding: TransferEncoding) -> Result<()> {
		self.write(b"Transfer-Encoding: ")?;
		self.write(encoding.as_str().as_bytes())?;
		return Ok(());
	}
	pub fn push_http_content_length(&mut self, length: usize) -> Result<()> {
		write!(self, "Content-Length: {}\r\n", length)?;
		return Ok(());
	}
	pub fn push_http_content_disposition(&mut self, mut disposition: ContentDisposition) -> Result<()> {
		disposition.read_into(self)?;
		self.write(b"\r\n")?;
		return Ok(());
	}
	pub fn begin_http_body(&mut self) -> Result<()> {
		self.write(b"\r\n")?;
		return Ok(());
	}

	pub fn push_http_body(&mut self, body: &[u8]) -> Result<()> {
		self.write(b"\r\n")?;
		self.write(body)?;
		return Ok(());
	}

	pub fn clear(&mut self) { self.filled = 0; }


	pub fn write_templated(
		&mut self,
		template: &[u8],
		keys: &[&str],
		values: &mut [&mut dyn ReadInto]
	) -> Result<()> {
		enum ParseState {
			None,
			Escaping,
			Key(usize)
		}

		let mut template_index = 0;
		let mut state = ParseState::None;
		'write: while template_index < template.len() {
			match state {
				ParseState::None => {
					if template[template_index] == b'%' {
						state = ParseState::Escaping;
						template_index += 1;
						continue 'write;
					}
					// self.buffer[self.filled] = template[template_index];
					self.write(&[template[template_index]])?;
					// self.filled += 0;
					template_index += 1;
				},
				ParseState::Escaping => {
					if template[template_index] == b'%' {
						// self.buffer[self.filled] = template[template_index];
						self.write(b"%")?;
						// self.filled += 1;
						template_index += 1;
						state = ParseState::None;
						continue 'write;
					}else {
						state = ParseState::Key(template_index);
						template_index += 1;
						continue 'write;
					}
				},
				ParseState::Key(key_start) => {
					if template[template_index] == b'%' {
						let template_key = &template[key_start..template_index];
						match keys.iter().enumerate().find(|(_iter, key)| key.as_bytes() == template_key) {
							Some((index, _key)) => { values[index].read_into(self)?; }
							None => {
								println!("\rDBG: failed to find key |-{}-|", unsafe{ template_key.as_ascii_unchecked() }.as_str());
								self.write(b"UNMATCHED KEY: ")?;
								self.write(template_key)?;
							}
						};
						state = ParseState::None;
					}
					template_index += 1;
				}
			}
		}

		return Ok(());
	}
}

impl <'a> Write for StreamBuffer<'a> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		for byte in buf {
			if self.filled == self.buffer.len() { self.flush()?; }
			self.buffer[self.filled] = *byte;
			self.filled += 1;
		}
		return Ok(buf.len());
	}

	fn flush(&mut self) -> std::io::Result<()> {
		self.stream.write_all(&self.buffer[0..self.filled])?;
		// self.stream.write(buf)
		self.stream.flush()?;
		self.filled = 0;
		return Ok(());
	}
}

pub trait ReadInto {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize>;
}

impl ReadInto for &[u8] {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		return destination.write(self).map_err(|e| e.into());
	}
}

impl ReadInto for &mut [&[u8]] {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		let mut read_size = 0;
		for string in self.iter_mut() {
			read_size += string.read_into(destination)?;
		}
		return Ok(read_size);
	}
}

impl ReadInto for usize {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		let mut digit_buffer: [u8; 24] = unsafe{ std::mem::zeroed() };
		let mut buffer_index = 0;

		let mut number = self.clone();
		while number > 0 {
			let digit = number % 10;
			digit_buffer[buffer_index] = digit as u8 + b'0';
			buffer_index += 1;

			number -= digit;
			number /= 10;
		}

		for digit in digit_buffer[0..buffer_index].iter().rev() {
			destination.write(&[*digit])?;
		}
		return Ok(buffer_index);
	}
}

macro_rules! block_read_into {
	($members: tt, $destination: ident) => {
		(&mut $members[0..]).read_into($destination)
	};
}

// ARGS
// item_name
// iter
// block
macro_rules! iterator_reader {
	($item_name: ident, $iter: expr, $block: tt) => {
		ClosureReader{ source: &|dest_writer| {
			let mut write_size = 0;
			for $item_name in $iter {
				write_size += block_read_into!($block, dest_writer)?;
			}
			return Ok(write_size);
		}}
	};
}


pub struct ClosureReader<'a> {
	pub source: &'a dyn Fn(&mut dyn Write) -> Result<usize>
}

impl ReadInto for ClosureReader<'_> {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		return (self.source)(destination);
	}
}


pub fn make_http_request(
	buffer: &mut StreamBuffer,
	method: HttpMethod,
	route: &str,
	headers: &[HttpHeader],
) -> Result<()> {
	write!(buffer, "{} {} HTTP/1.1", method.as_str(), route)?;
	for header in headers { header.clone().read_into(buffer)?; }
	buffer.write(b"\r\n\r\n")?;
	
	return Ok(());
}




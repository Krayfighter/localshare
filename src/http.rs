
use std::io::{ Write, Read };

use anyhow::Result;


#[derive(Debug)]
pub enum HttpRequestType {
	GET, POST, ERASE,
}

#[allow(non_camel_case_types)]
pub enum ContentType {
	text_html,
	text_plain,
	image_x_icon,
	audio_flac
}

#[allow(non_camel_case_types)]
pub enum TransferEncoding {
	_7bit,
	_8bit,
	binary,
	quoted_printable,
	base64
}

pub enum ContentDisposition<'a> {
	Inline,
	Attachment(Option<&'a str>)
}


pub struct StreamBuffer<'a> {
	buffer: &'a mut [u8],
	filled: usize,
	stream: &'a mut dyn Write
}

impl<'a> StreamBuffer<'a> {
	pub fn new(buffer: &'a mut [u8], stream: &'a mut dyn Write) -> Self {
		return Self{ buffer, stream, filled: 0 };
	}
	pub fn push_http_primary_header<
		A: AsRef<[u8]>, B: AsRef<[u8]>
	>(&mut self, protocol_version: A, code: usize, status: B) -> Result<()> {
		self.write(protocol_version.as_ref())?;
		write!(self, " {code} ")?;
		self.write(status.as_ref())?;
		self.write(b"\r\n")?;
		return Ok(());
	}
	pub fn push_http_content_type(&mut self, ctype: ContentType) -> Result<()> {
		self.write(b"Content-Type: ")?;
		match ctype {
			ContentType::text_html => self.write(b"text/html")?,
			ContentType::text_plain => self.write(b"text/plain")?,
			ContentType::image_x_icon => self.write(b"image/x-icon")?,
			ContentType::audio_flac => self.write(b"audio/flac")?,
		};
		self.write(b"\r\n")?;
		return Ok(());
	}
	pub fn push_http_transfer_encoding(&mut self, encoding: TransferEncoding) -> Result<()> {
		self.write(b"Transfer-Encoding: ")?;
		match encoding {
			TransferEncoding::_7bit => self.write(b"7bit")?,
			TransferEncoding::_8bit => self.write(b"8bit")?,
			TransferEncoding::binary => self.write(b"binary")?,
			TransferEncoding::quoted_printable => self.write(b"quoted_printable")?,
			TransferEncoding::base64 => self.write(b"base64")?,
		};
		return Ok(());
	}
	pub fn push_http_content_length(&mut self, length: usize) -> Result<()> {
		write!(self, "Content-Length: {}\r\n", length)?;
		return Ok(());
	}
	pub fn push_http_content_disposition(&mut self, disposition: ContentDisposition) -> Result<()> {
		self.write(b"Content-Disposition: ")?;
		match disposition {
			ContentDisposition::Inline => { self.write(b"inline")?; },
			ContentDisposition::Attachment(None) => { write!(self, "attachment")?; },
			ContentDisposition::Attachment(Some(filename)) => {
				write!(self, "attachment; filename={}; filename*={}", filename, filename)?;
			}
		};
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

impl<'a> Write for StreamBuffer<'a> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		for byte in buf {
			if self.filled == self.buffer.len() { self.flush()?; }
			self.buffer[self.filled] = *byte;
			self.filled += 1;
		}
		return Ok(buf.len());
	}

	fn flush(&mut self) -> std::io::Result<()> {
		self.stream.write(&self.buffer[0..self.filled])?;
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





use std::{any::Any, io::{ Read, Write }};

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
	pub fn from_str(source: &str) -> Option<Self> {
		match source {
			"GET" => Some(Self::GET),
			"POST" => Some(Self::POST),
			"ERASE" => Some(Self::ERASE),
			_ => None
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
	pub fn from_str_pair(key: &str, value: &'a str) -> Result<Option<Self>> {
		match key {
			"Content-Type" => {
				if let Some(ctype) = ContentType::from_str(value) {
					return Ok(Some(HttpHeader::ContentType(ctype)));
				}else { bail!("unrecognized content type"); }
			},
			"Transfer-Encoding" => {
				if let Some(encoding) = TransferEncoding::from_str(value) {
					return Ok(Some(HttpHeader::TransferEncoding(encoding)));
				}else { bail!("unrecognized transfer encoding"); }
			},
			"Content-Length" => {
				if let Ok(len) = value.parse::<usize>() {
					return Ok(Some(HttpHeader::ContentLength(len)));
				} else { bail!("invalid content length"); }
			},
			"Content-Disposition" => { todo!() },
			// NOTE these are a list of recognized but unhandled http header keys
			"Accept-Language" | "Range" | "DNT" | "Sec-GPC"
				| "Connection" | "Referer" | "Sec-Fetch-Dest"
				| "Sec-Fetch-Mode" | "Sec-Fetch-Site" | "Accept-Encoding"
				| "Priority" | "Accept" | "Host" | "User-Agent"
				| "Upgrade-Insecure-Requests" => {
					Ok(None)
				}
			_ => bail!("unrecognized HTTP header"),
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
		// write_size += destination.write(b"\r\n")?;
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
	// pub fn push_http_response_primary_header<
	// 	A: AsRef<[u8]>, B: AsRef<[u8]>
	// >(&mut self, protocol_version: A, code: usize, status: B) -> Result<()> {
	// 	self.write(protocol_version.as_ref())?;
	// 	write!(self, " {code} ")?;
	// 	self.write(status.as_ref())?;
	// 	self.write(b"\r\n")?;
	// 	return Ok(());
	// }
	// pub fn push_http_request_primary_header(
	// 	&mut self, method: HttpMethod, route: &str, protocol_version: &str
	// ) -> Result<()> {
	// 	write!(self, "{} {} {}", method.as_str(), route, protocol_version)?;
	// 	return Ok(());
	// }
	// pub fn push_http_content_type(&mut self, ctype: ContentType) -> Result<()> {
	// 	self.write(b"Content-Type: ")?;
	// 	self.write(ctype.as_str().as_bytes())?;
	// 	self.write(b"\r\n")?;
	// 	return Ok(());
	// }
	// pub fn push_http_transfer_encoding(&mut self, encoding: TransferEncoding) -> Result<()> {
	// 	self.write(b"Transfer-Encoding: ")?;
	// 	self.write(encoding.as_str().as_bytes())?;
	// 	return Ok(());
	// }
	// pub fn push_http_content_length(&mut self, length: usize) -> Result<()> {
	// 	write!(self, "Content-Length: {}\r\n", length)?;
	// 	return Ok(());
	// }
	// pub fn push_http_content_disposition(&mut self, mut disposition: ContentDisposition) -> Result<()> {
	// 	disposition.read_into(self)?;
	// 	self.write(b"\r\n")?;
	// 	return Ok(());
	// }
	// pub fn begin_http_body(&mut self) -> Result<()> {
	// 	self.write(b"\r\n")?;
	// 	return Ok(());
	// }

	// pub fn push_http_body(&mut self, body: &[u8]) -> Result<()> {
	// 	self.write(b"\r\n")?;
	// 	self.write(body)?;
	// 	return Ok(());
	// }

	pub fn clear(&mut self) { self.filled = 0; }
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
		self.stream.flush()?;
		self.filled = 0;
		return Ok(());
	}
}

pub trait ReadInto {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize>;
}

impl ReadInto for &mut [&mut dyn ReadInto] {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		let mut write_size = 0;
		for object in self.iter_mut() {
			write_size += object.read_into(destination)?;
		}
		return Ok(write_size);
	}
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

pub struct ClosureReader<'a> {
	pub source: &'a dyn Fn(&mut dyn Write) -> Result<usize>
}

impl ReadInto for ClosureReader<'_> {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		return (self.source)(destination);
	}
}

pub struct BodyTemplate<'a> {
	pub template: &'a [u8],
	pub keys: &'a [&'a [u8]],
	pub values: &'a mut [&'a mut dyn ReadInto],
}

impl ReadInto for BodyTemplate<'_> {
	fn read_into(&mut self, destination: &mut dyn Write) -> Result<usize> {
		assert!(self.keys.len() == self.values.len());
		enum ParseState {
			None,
			Escaping,
			Key(usize)
		}

		let mut write_size = 0;
		let mut template_index = 0;
		let mut state = ParseState::None;
		'write: while template_index < self.template.len() {
			match state {
				ParseState::None => {
					if self.template[template_index] == b'%' {
						state = ParseState::Escaping;
						template_index += 1;
						continue 'write;
					}
					write_size += destination.write(&[self.template[template_index]])?;
					template_index += 1;
				},
				ParseState::Escaping => {
					if self.template[template_index] == b'%' {
						write_size += destination.write(b"%")?;
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
					if self.template[template_index] == b'%' {
						let template_key = &self.template[key_start..template_index];
						match self.keys.iter().enumerate().find(|(_iter, key)| **key == template_key) {
							Some((index, _key)) => { self.values[index].read_into(destination)?; }
							None => {
								println!("\rDBG: failed to find key |-{}-|", unsafe{ template_key.as_ascii_unchecked() }.as_str());
								write_size += destination.write(b"UNMATCHED KEY: ")?;
								write_size += destination.write(template_key)?;
							}
						};
						state = ParseState::None;
					}
					template_index += 1;
				}
			}
		}

		return Ok(write_size);
	}
}


#[derive(Clone)]
pub struct HttpRequest<'a> {
	pub protocol_version: &'a str,
	pub method: HttpMethod,
	pub route: &'a str,
	pub query_params: &'a str,
	pub headers: Vec<HttpHeader<'a>>,
	pub body: &'a [u8],
}

impl HttpRequest<'_> {
	// NOTE expects source to be non-blocking
	pub fn read_blocking<'a>(
		buffer: &'a mut Vec<u8>,
		source: &mut dyn std::io::Read,
	) -> Result<HttpRequest<'a>> {
		// TODO this may still be incorrect
		let mut intermediate_buffer: [u8; 16384] = unsafe{ std::mem::zeroed() };
		'preliminary_read: loop {
			match source.read(&mut intermediate_buffer) {
				Ok(count) => {
					buffer.write_all(&intermediate_buffer[..count]).unwrap();
					break 'preliminary_read;
				},
				Err(e) => { match e.kind() {
					std::io::ErrorKind::WouldBlock => {
						std::thread::sleep(std::time::Duration::from_millis(10));
						continue 'preliminary_read;
					},
					_ => {
						println!("\rWARN: failed to read from tcp socket -> {e}");
						break 'preliminary_read;
					}
				}}
			}
		}

		println!("\rDBG: request buffer contents: {}", unsafe{ buffer.as_slice().as_ascii_unchecked() }.as_str());
		if buffer.len() == 0 { bail!("0 bytes read from request source"); }

		let (head, body) = match crate::split_slice_uninclusive(buffer.as_slice(), b"\r\n\r\n") { 
			Some(pair) => pair,
			None => {
				// println!("\rWARN: did not receive body separator from http request in time");
				(&buffer.as_slice()[..buffer.len()-4], &b""[..])
			}
		};
		let head = unsafe{ head.as_ascii_unchecked() }.as_str();

		let mut headers = Vec::<crate::http::HttpHeader>::new();
		let mut header_iter = head.split("\r\n");
		let primary_header = header_iter.next()
			.ok_or(anyhow!("Http request without primary header"))?;
		for line in header_iter {
			let (key, value) = match line.split_once(": ") {
				Some(pair) => pair,
				None => {
					println!("\rWARN: malformed http header (missing value) -> {}", line);
					continue;
				}
			};
			match crate::http::HttpHeader::from_str_pair(key, value) {
				Ok(Some(header)) => {
					headers.push(header);
				},
				Ok(None) => {
					// println!("\rWARN: failed to parse http header | key: {} - value: {} |", key, value);
				},
				Err(e) => {
					println!("\rWARN: failed to parse http header -> {e} - {}", line);
				}
			}
		}

		let mut primary_header_segments = primary_header.split(" ");
		let method_str = primary_header_segments.next()
			.ok_or(anyhow!("malformed http primary header - no method"))?;
		let uri_str = primary_header_segments.next()
			.ok_or(anyhow!("malformed http primary header - no route"))?;
		let version_str = primary_header_segments.next()
			.ok_or(anyhow!("malformed http primary header - no version"))?;

		let (route_str, query_param_str) = uri_str.split_once("?")
			.unwrap_or((uri_str, ""));

		let method = HttpMethod::from_str(method_str)
			.ok_or(anyhow!("invalid http method"))?;
	
		return Ok(HttpRequest {
			protocol_version: version_str,
			route: route_str,
			query_params: query_param_str,
			headers,
			body,
			method
		});
	}

	fn write_headers_to(&self, sink: &mut dyn Write) -> Result<()> {
		sink.write(self.method.as_str().as_bytes())?;
		sink.write(b" ")?;
		sink.write(self.route.as_bytes())?;
		if self.query_params != "" {
			sink.write(b"?")?;
			sink.write(self.query_params.as_bytes())?;
		}
		sink.write(b" ")?;
		sink.write(self.protocol_version.as_bytes())?;
		sink.write(b"\r\n")?;
		
		for header in self.headers.iter() {
			let mut header = *header;
			header.read_into(sink)?;
			sink.write(b"\r\n")?;
		}

		return Ok(());
	}

	pub fn write_to_sink(&self, sink: &mut dyn Write) -> Result<()> {
		self.write_headers_to(sink)?;
		sink.write(b"\r\n")?;

		sink.write_all(self.body)?;

		return Ok(());
	}

	pub fn write_from_readinto(&self, source: &mut dyn ReadInto, sink: &mut dyn Write) -> Result<()> {
		self.write_headers_to(sink)?;
		sink.write(b"\r\n")?;

		source.read_into(sink)?;

		return Ok(());
	}
}

pub struct HttpResponse<'a> {
	pub protocol_version: &'a str,
	pub status_code: usize,
	pub status_text: &'a str,
	pub headers: Vec<HttpHeader<'a>>,
	pub body: &'a [u8]
}

impl HttpResponse<'_> {
	pub fn read_blocking<'a>(
		buffer: &'a mut Vec<u8>,
		source: &mut dyn std::io::Read,
	) -> Result<HttpResponse<'a>> {
		// TODO this may still be incorrect
		let mut intermediate_buffer: [u8; 16384] = unsafe{ std::mem::zeroed() };
		'preliminary_read: loop {
			match source.read(&mut intermediate_buffer) {
				Ok(count) => {
					buffer.write_all(&intermediate_buffer[..count]).unwrap();
					break 'preliminary_read;
				},
				Err(e) => { match e.kind() {
					std::io::ErrorKind::WouldBlock => {
						std::thread::sleep(std::time::Duration::from_millis(10));
						println!("\rDBG: encountered nonblocking stream in response reader");
						continue 'preliminary_read;
					},
					_ => {
						println!("\rWARN: failed to read from tcp socket -> {e}");
						break 'preliminary_read;
					}
				}}
			}
		}

		println!("\rDBG: no body -> buffer len = {}", buffer.len());
		println!("\rDBG: body string -> {}", unsafe{ buffer.as_slice().as_ascii_unchecked() }.as_str() );
		if buffer.len() == 0 { bail!("Failed to read any bytes from source, even in blocking mode"); }

		let (head, body) = match crate::split_slice_uninclusive(buffer.as_slice(), b"\r\n\r\n") { 
			Some(pair) => pair,
			None => {
				// println!("\rWARN: did not receive body separator from http request in time");
				(&buffer.as_slice()[..buffer.len()-4], &b""[..])
			}
		};
		let head = unsafe{ head.as_ascii_unchecked() }.as_str();

		let mut headers = Vec::<crate::http::HttpHeader>::new();
		let mut header_iter = head.split("\r\n");
		let primary_header = header_iter.next()
			.ok_or(anyhow!("Http request without primary header"))?;
		for line in header_iter {
			let (key, value) = match line.split_once(": ") {
				Some(pair) => pair,
				None => {
					println!("\rWARN: malformed http header (missing value) -> {}", line);
					continue;
				}
			};
			match crate::http::HttpHeader::from_str_pair(key, value) {
				Ok(Some(header)) => {
					headers.push(header);
				},
				Ok(None) => {
					// println!("\rWARN: failed to parse http header | key: {} - value: {} |", key, value);
				},
				Err(e) => {
					println!("\rWARN: failed to parse http header -> {e} - {}", line);
				}
			}
		}

		let mut primary_header_segments = primary_header.split(" ");
		let version_str = primary_header_segments.next()
			.ok_or(anyhow!("malformed http primary header - no method"))?;
		let status_code = primary_header_segments.next()
			.ok_or(anyhow!("malformed http primary header - no route"))?
			.parse::<usize>()?;
		let status_text = primary_header_segments.next()
			.ok_or(anyhow!("malformed http primary header - no version"))?;

		// let (route_str, query_param_str) = uri_str.split_once("?")
		// 	.unwrap_or((uri_str, ""));

		// let method = HttpMethod::from_str(method_str)
		// 	.ok_or(anyhow!("invalid http method"))?;
	
		return Ok(HttpResponse{
			protocol_version: version_str,
			headers,
			body,
			status_code,
			status_text,
		});
	}

	fn write_headers_to(&self, sink: &mut dyn Write) -> Result<()> {
		sink.write(self.protocol_version.as_bytes())?;
		sink.write(b" ")?;
		self.status_code.clone().read_into(sink)?;
		sink.write(b" ")?;
		sink.write(self.status_text.as_bytes())?;
		sink.write(b"\r\n")?;

		for header in self.headers.iter() {
			let mut header = *header;
			header.read_into(sink)?;
			sink.write(b"\r\n")?;
		}

		return Ok(());
	}

	pub fn write_to_sink(&self, sink: &mut dyn Write) -> Result<()> {
		self.write_headers_to(sink)?;
		sink.write(b"\r\n")?;
		sink.flush()?;

		sink.write_all(self.body)?;

		return Ok(());
	}

	pub fn write_from_readinto(&self, source: &mut dyn ReadInto, sink: &mut dyn Write) -> Result<()> {
		self.write_headers_to(sink)?;
		sink.write(b"\r\n")?;

		source.read_into(sink)?;

		return Ok(());
	}
}


macro_rules! block_read_into {
	($members: tt, $destination: ident) => {
		(&mut $members[..]).read_into($destination)
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


#[cfg(test)]
mod http_tools_test {
	#[test]
	fn test_http_response() {
		let mut output = Vec::<u8>::new();

		let example_body1 = b"<h1>Welcome</h1><p>the paragraph</p>";

		let response_structs: &[super::HttpResponse] = &[
			super::HttpResponse {
				protocol_version: "HTTP/1.1",
				status_code: 200,
				status_text: "OK",
				headers: vec![
					super::HttpHeader::ContentType(super::ContentType::text_html),
					super::HttpHeader::ContentLength(example_body1.len()),
				],
				body: example_body1,
			},
		];
		let response_strings: &[&[u8]] = &[
			b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 36\r\n\r\n<h1>Welcome</h1><p>the paragraph</p>"
		];

		assert_eq!(response_structs.len(), response_strings.len());
		for index in 0..response_structs.len() {
			output.clear();
			response_structs[index].write_to_sink(&mut output).unwrap();
			assert_eq!(
				unsafe{ output.as_slice().as_ascii_unchecked() }.as_str(),
				unsafe{ response_strings[index].as_ascii_unchecked() }.as_str()
			);
		}
	}

	#[test]
	fn test_http_request() {
		let mut buffer = Vec::<u8>::new();
		let req = super::HttpRequest {
			protocol_version: "HTTP/1.1",
			method: crate::http::HttpMethod::GET,
			route: "/files",
			query_params: "",
			headers: vec!(),
			body: b"",
		};
		let req_str = b"GET /files HTTP/1.1\r\n\r\n";

		req.write_to_sink(&mut buffer).unwrap();
		assert_eq!(buffer.as_slice(), req_str);
	}
}


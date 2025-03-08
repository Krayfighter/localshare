#![feature(tcplistener_into_incoming)]
#![feature(ptr_as_ref_unchecked)]
// #![feature(static_mut_refs)]
// #![feature(lock_value_accessors)]

use std::io::{Read, Write};

use std::sync::{Mutex, Arc};

use anyhow::{anyhow, Result};
#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate crossterm;

struct FileDatabase {
	filenames: Mutex<Vec<Arc<str>>>,
	file_contents: Mutex<Vec<Arc<str>>>,
}

impl FileDatabase {
	pub fn new() -> Self {
		return Self {
			filenames: Mutex::new(Vec::new()),
			file_contents: Mutex::new(Vec::new())
		};
	}
}

static mut FILE_ENTRIES: Option<Arc<FileDatabase>> = None;
static mut FAVICON: Option<Arc<[u8]>> = None;

fn get_file_entry_names() -> Vec<Arc<str>> {
	#[allow(static_mut_refs)]
	let entries = unsafe{ FILE_ENTRIES.clone().unwrap() };

	let filenames = entries.filenames.lock().unwrap().clone();

	return filenames;
}

fn get_file_entry_by_name(name: &str) -> Option<Arc<str>> {
	#[allow(static_mut_refs)]
	let entries = unsafe{ FILE_ENTRIES.clone().unwrap() };

	let filename_lock = entries.filenames.lock().unwrap();
	let filecont_lock = entries.file_contents.lock().unwrap();

	let (index, _filename) = match filename_lock.clone()
		.into_iter()
		.enumerate()
		.find(|(_iter, filename)| filename.as_ref() == name) {
		Some(pair) => pair,
		None => return None
	};

	return filecont_lock.get(index).cloned();
}

fn push_file_entry(name: &str, contents: &str) {
	#[allow(static_mut_refs)]
	let entries = unsafe{ FILE_ENTRIES.clone().unwrap() };

	let mut name_lock = entries.filenames.lock().unwrap();
	let mut content_lock = entries.file_contents.lock().unwrap();

	name_lock.push(Arc::from(name));
	content_lock.push(Arc::from(contents));
}

// const filething_addr = unsafe{
// 	std::ptr::dangling_mut()
// }

fn convert_c_string(string: &[u8]) -> String {
	let mut output = String::new();
	output.reserve(string.len());

	for byte in string {
		if *byte == 0 { break; }
		output.push(*byte as char);
	}

	return output;
}


#[derive(Debug)]
enum HttpRequestType {
	GET, POST, ERASE,
}

#[allow(non_camel_case_types)]
enum ContentType {
	text_html,
	text_plain,
	image_x_icon
}

#[allow(non_camel_case_types)]
enum TransferEncoding {
	_7bit,
	_8bit,
	binary,
	quoted_printable,
	base64
}

enum ContentDisposition<'a> {
	Inline,
	Attachment(Option<&'a str>)
}

struct StreamBuffer<'a> {
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
		// write!(self, "{} {code} {}\r\n", version.as_ref(), status.as_ref())?;
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
			ContentType::image_x_icon => self.write(b"image/x-icon")?
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


fn handle_client(mut client: std::net::TcpStream) -> Result<()> {
	let client_peer_addr = client.peer_addr()?;
	let client_local_addr = client.local_addr()?;
	print!("INFO: serving http request from {:?} for route", client_peer_addr);
	let mut buffer: [u8; 4096] = unsafe{ std::mem::zeroed() };

	let request_type: HttpRequestType;
	let request_uri: &str;

	client.read(&mut buffer)?;
	let buffer_string = convert_c_string(&buffer);

	let mut header_iterator = buffer_string.split("\r\n");
	let request_line = header_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;
	let mut request_line_iterator = request_line.split(" ");
	let request_type_string = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	if request_type_string == "GET" { request_type = HttpRequestType::GET; }
	else if request_type_string == "POST" { request_type = HttpRequestType::POST; }
	else { bail!("Unhandled HTTP request type"); }

	print!(" {:?}", request_type);

	request_uri = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	println!(" {:?}", request_uri);

	let http_version = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	let mut iobuf: [u8; 4096] = unsafe{ std::mem::zeroed() };
	let mut buffer = StreamBuffer::new(&mut iobuf, &mut client);


	// buffer.write(http_version.as_bytes())?;

	let mut uri_iter = request_uri.split('?');
	let uri_path = uri_iter.next().or(Some("/")).unwrap();
	let uri_query = uri_iter.next();

	if uri_path == "/" {
		buffer.push_http_primary_header(http_version, 200, "OK")?;
		// buffer.write(b" 200 OK\r\n")?;
		// buffer.write(b"Content-Type: text/html\n\r\n\r")?;
		buffer.push_http_content_type(ContentType::text_html)?;
		buffer.write(b"\r\n")?;

		buffer.write(b"<body><h2>Home Page</h2>")?;
		write!(buffer, "<div style=\"float: left; padding-right: 5%;\">Local Addr:<br />Peer Addr:</div><div>{:#?}<br />{:#?}</div>", client_local_addr, client_peer_addr)?;
		let filenames = get_file_entry_names();
		if filenames.len() == 0 {
			buffer.write(b"<h3>No Files Selected Yet</h3>")?;
		}else {
			buffer.write(b"<h3>Available Files</h3>")?;
			for filename in filenames {
				buffer.write(b"<a href=/file/")?;
				buffer.write(filename.as_bytes())?;
				buffer.write(b">")?;
				buffer.write(filename.as_bytes())?;
				buffer.write(b"</a><br />")?;
			}
		}
	}
	else if uri_path == "/favicon.ico" {
		buffer.push_http_primary_header(http_version, 200, "OK")?;
		buffer.push_http_content_type(ContentType::image_x_icon)?;
		buffer.push_http_transfer_encoding(TransferEncoding::binary)?;

		#[allow(static_mut_refs)]
		let favicon_slice = unsafe{ FAVICON.clone().unwrap() };
		buffer.push_http_content_length(favicon_slice.len())?;
		buffer.write(b"\r\n")?;
		buffer.write(favicon_slice.as_ref())?;
	}
	else if uri_path.starts_with("/file/") {

		let filepath = &uri_path[6..];

		let result = get_file_entry_by_name(filepath);
		if let Some(file) = result {
			buffer.push_http_primary_header(http_version, 200, "OK")?;
			buffer.push_http_content_type(ContentType::text_plain)?;
			buffer.push_http_transfer_encoding(TransferEncoding::binary)?;
			buffer.push_http_content_length(file.len())?;
			buffer.push_http_content_disposition(ContentDisposition::Attachment(Some(
				filepath.split('/').rev().next().unwrap_or(filepath)
			)))?;
			buffer.write(b"\"\r\n")?;
			buffer.write(file.as_bytes())?;
		}else {
			// buffer.write(b" 404 Not Found\r\n")?;
			buffer.push_http_primary_header(http_version, 404, "Not Found")?;
			// buffer.write(b"Content-Type: text/html\r\n\r\n")?;
			buffer.push_http_content_type(ContentType::text_html)?;
			buffer.write(b"<body><h2>404 - File Note Found</h2></body>")?;
		}
	}
	else {
		println!("INFO: received http request for undefined route {}", request_uri);
		buffer.write(b" 404 Not Found\r\n\r\n")?;
		buffer.write(b"<h3>This route is not defined - 404 Not Found</h3>")?;
	}

	buffer.flush()?;
	client.shutdown(std::net::Shutdown::Both)?;

	return Ok(());
}


fn main() {
	unsafe { FILE_ENTRIES = Some(Arc::new(FileDatabase::new())) };
	{
		let favicon = std::fs::read("favicon.ico").expect("Failed to open favicon.ico");
		unsafe { FAVICON = Some(Arc::from(favicon)); }
	}
	
	if let Ok(filebytes) = std::fs::read("entries.txt") {
		let filestring = convert_c_string(&filebytes);
		for line in filestring.split('\n') {
			if let Ok(entry_string) = std::fs::read(line) {
				push_file_entry(line, &convert_c_string(&entry_string));
			}else { println!("Failed to open entry from file"); }
		}
	}else { println!("Failed to open entries file"); }


	let listener = std::net::TcpListener::bind(
		std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
			// std::net::Ipv4Addr::new(192, 168, 12, 182),
			std::net::Ipv4Addr::UNSPECIFIED,
			8000
		))
	).expect("Failed to bind tcp listener to localhost");

	println!("INFO: serving at local addr: {:?}", listener.local_addr());


	let listener_thread_handle = std::thread::spawn(move || {
		let mut threads = Vec::<std::thread::JoinHandle<()>>::new();
		listener.set_nonblocking(true).expect("Failed to set listener socket to nonblocking");

		loop {
			if let Ok((stream, _addr)) = listener.accept() {
				threads.push(std::thread::spawn(move || { let _ = handle_client(stream); } ))
			}else {
				let mut index = 0;
				while index < threads.len() {
					if threads[index].is_finished() {
						threads.swap_remove(index).join().expect("Failed to join finished thread");
					}
					index += 1;
				}
				std::thread::sleep(std::time::Duration::from_millis(10))
			}
		}
	});



	// let stdin = std::io::stdin();
	crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");

	// ctrlc::set_handler(|| {
	// 	crossterm::terminal::disable_raw_mode().expect("Failed to exit raw mode");
	// }).expect("Failed to set Ctrl-C(ancel) signal handler");

	let mut stdout = std::io::stdout();
	let mut buffer = String::new();
	'user_mainloop: loop {

		'input: loop {
			// stdout.write(b"\r> ").unwrap();
			// std1
			queue!(stdout,
				crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)
			).expect("Failed to queue to stdout");
			write!(stdout, "\r> {}", buffer).expect("Failed to write to stdout");
			stdout.flush().unwrap();

			if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(10)) {
				match crossterm::event::read() {
					Ok(crossterm::event::Event::Key(crossterm::event::KeyEvent {
						code: crossterm::event::KeyCode::Esc, modifiers: crossterm::event::KeyModifiers::NONE,
						kind: crossterm::event::KeyEventKind::Press, state: _
					})) => break 'input,
					Ok(crossterm::event::Event::Key(key)) => {
						if key.modifiers == crossterm::event::KeyModifiers::CONTROL {
							match key.code {
								crossterm::event::KeyCode::Char('c') => {
									if buffer.is_empty() { break 'user_mainloop; }
									else { buffer.clear(); }
								},
								crossterm::event::KeyCode::Char('w') => { buffer.clear(); }
								_ => {},
							}
						}else {
							match key.code {
								crossterm::event::KeyCode::Char(chr) => { buffer.push(chr); },
								crossterm::event::KeyCode::Esc => break 'user_mainloop,
								crossterm::event::KeyCode::Backspace => { buffer.pop(); }
								crossterm::event::KeyCode::Enter => break 'input,
								_ => {},
							};
						}
					},
					Err(e) => panic!("Failed to read crossterm event: {}", e),
					_ => {}
				}
			}
			
		}
		stdout.write(b"\n").expect("Failed to write to stdout");
		// let read_size = stdin.read_line(&mut buffer).expect("Failed to read from stdin");
		let line_str = &buffer.as_str()[0..];

		if line_str == "quit" { break; }
		else if line_str == "clear" {
			queue!(stdout,
				crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
				crossterm::cursor::MoveTo(0, 0)
			).expect("Failed to queue to stdout");
		}else if line_str == "help" {
			todo!()
		}else if line_str.starts_with("add ") {
			let filename = &line_str[4..];
			if std::fs::exists(filename).expect("Failed to check file existence") {
				println!("INFO: adding file {} to database", filename);
				push_file_entry(filename, &convert_c_string(&std::fs::read(filename).expect("Failed to read from file")));
			}else {
				println!("ERR: unable to locate file {}", filename);
			}
		}
		buffer.clear();
	}


	let mut file = std::fs::File::create("entries.txt").expect("Failed to open entries file for saving");
	for entry in get_file_entry_names() {
		file.write(entry.as_bytes()).expect("failed to write to file");
		file.write(b"\n").expect("failed to write to file");
	}
	
	crossterm::terminal::disable_raw_mode().expect("Failed to exit raw mode");

	return;
}


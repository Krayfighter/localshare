#![feature(tcplistener_into_incoming)]
#![feature(ptr_as_ref_unchecked)]
// #![feature(static_mut_refs)]
// #![feature(lock_value_accessors)]

use std::io::{Read, Write};

use std::sync::{Arc, Mutex, RwLock};

use anyhow::{anyhow, Result};
#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate crossterm;



struct FileDatabase {
	filenames: Vec<Arc<str>>,
	file_contents: Vec<Arc<memmap2::Mmap>>
}

impl FileDatabase {
	pub fn new() -> Self {
		return Self {
			filenames: Vec::new(),
			file_contents: Vec::new()
		};
	}
}

struct Playlist {
	name: Arc<str>,
	files: FileDatabase,
}

struct Globals {
	file_entries: Option<RwLock<FileDatabase>>,
	playlists: Option<RwLock<Vec<Playlist>>>,
	// playlists: Option<Mutex<Vec<FileDatabase>>>,
	// playlist_names: Option<Mutex<Vec<Arc<str>>>>,
	// favicon: Option<&'static [u8]>,
	favicon: Option<memmap2::Mmap>,
}

impl Globals {
	pub fn get_file_entry_names(&self) -> Vec<Arc<str>> {
		let entries = self.file_entries.as_ref().unwrap();
		let filenames = entries.read().unwrap().filenames.clone();

		return filenames;
	}

	pub fn get_file_entry_by_name(&self, name: &str) -> Option<Arc<memmap2::Mmap>> {
		let entries = self.file_entries.as_ref().unwrap().read().unwrap();

		// let filename_lock = entries.filenames;
		// let filecont_lock = entries.read().unwrap().file_contents;

		let (index, _filename) = match entries.filenames
			.iter()
			.enumerate()
			.find(|(_iter, filename)| filename.as_ref() == name) {
			Some(pair) => pair,
			None => return None
		};

		return entries.file_contents.get(index).cloned();
	}

	// pub fn push_file_entry(&self, name: &str, contents: &str) {
	pub fn push_file_entry<P: AsRef<std::path::Path>>(&self, name: &str, fpath: P) -> Result<()> {
		// #[allow(static_mut_refs)]

		let file = std::fs::File::open(fpath.as_ref())?;
		let filemap = unsafe{ memmap2::Mmap::map(&file) }?;

		// let mut name_lock = entries.filenames.lock().unwrap();
		// let mut content_lock = entries.file_contents.lock().unwrap();

		let mut entries = self.file_entries.as_ref().unwrap().write().unwrap();

		entries.filenames.push(Arc::from(name));
		entries.file_contents.push(Arc::from(filemap));

		return Ok(());
	}

	pub fn push_playlist_directory(&self, dirname: &str) -> Result<()> {
		
		let playlist_name = dirname.split('/').rev().next()
			.unwrap_or(dirname);

		let mut playlist = FileDatabase::new();
		{
			// let mut playlist_filenames = playlist.filenames.lock().unwrap();
			// let mut playlist_files = playlist.file_contents.lock().unwrap();

			for entry in std::fs::read_dir(dirname)? {
				if let Ok(entry) = entry {
					if entry.file_type()?.is_file() {
						// if let file = std::fs::File::open(entry.path())?;
						match std::fs::File::open(entry.path()) {
							Ok(file) => {
								playlist.file_contents.push(Arc::from(unsafe{ memmap2::Mmap::map(&file) }?));
							},
							Err(e) => {
								println!("WARN: failed to add {} to playlist files", e);
							}
						}

						playlist.filenames.push(Arc::from(
							entry.file_name().to_str().expect("Failed to convert OsString to &str")
						));
					}
				}
			}
		}

		let mut playlists = self.playlists.as_ref().unwrap().write().unwrap();

		// let mut playlists = self.playlists.as_ref().unwrap().lock().expect("Failed to lock playlists");
		// let mut playlist_names = self.playlist_names.as_ref().unwrap().lock().expect("Failed to lock playlist names");

		// if playlists.len() != playlist_names.len() { panic!("Invalid lock state"); }

		playlists.push(Playlist{ name: Arc::from(playlist_name), files: playlist });
		// playlist_names.push(Arc::from(playlist_name));

		return Ok(());
	}

	pub fn get_song_by_playlist_and_index(&self, playlist_name: &str, song_number: u32) -> Option<Arc<memmap2::Mmap>> {
		// let playlists_lock = GLOBALS.playlists.as_ref().unwrap().lock()
		// 	.expect("Failed to lock playlists");
		// let playlist_names_lock = self.playlist_names.as_ref().unwrap().lock()
		// 	.expect("Failed to lock playlist names");
		let playlists = self.playlists.as_ref().unwrap().read().unwrap();

		let index = match playlists.iter()
			.map(|playlist| playlist.name.clone())
			.enumerate()
			.find(|(_iter, pname)| pname.as_ref() == playlist_name)
		{
			Some((index, _pname)) => index,
			None => return None
		};

		// let playlist_lock = self.playlists.as_ref().unwrap().lock()
		// 	.expect("Failed top lock playlists");

		// let playlist = playlists.get(index).unwrap();
		let playlist = match playlists.get(index) {
			Some(playlist) => playlist,
			None => return None
		};

		// let songmap_lock = playlist..lock()
		// 	.expect("Failed to long song files mutex");

		// let thing = playlist.files.file_contents
		let songmap = playlist.files.file_contents.get(song_number as usize);

		// return Some(Arc::from(playlist));
		// todo!()
		return songmap.cloned();
	}
}

static GLOBALS: std::sync::LazyLock<Globals> = std::sync::LazyLock::new(|| {
	// let favicon = std::fs::read("favicon.ico").expect("Failed to open favicon.ico");
	let favicon_file = std::fs::File::open("favicon.ico").expect("Failed to open favicon.ico");
	let favicon = unsafe{ memmap2::Mmap::map(&favicon_file) }.expect("Failed to map favicon.ico into memory");
	return Globals {
		file_entries: Some(RwLock::new(FileDatabase::new())),
		favicon: Some(favicon),
		playlists: Some(RwLock::new(Vec::new())),
	}
});






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
	image_x_icon,
	audio_flac
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
	pub fn push_http_header_tail(&mut self) -> Result<()> {
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


fn return_routing_error(buffer: &mut StreamBuffer, error_string: &str) {
	buffer.push_http_primary_header("HTTP/1.1", 400, "Routing Error").unwrap();
	buffer.push_http_header_tail().unwrap();
	write!(buffer, "<h2>Routing Error</h2><p>{}</p>", error_string).unwrap();
	buffer.flush().unwrap();
	// buffer.write(b"<h2>Routing Error</h2>").expect("");
}

fn serve_route_index(
	client_local_addr: std::net::SocketAddr,
	client_peer_addr: std::net::SocketAddr,
	buffer: &mut StreamBuffer
) -> Result<()> {
	buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
	// buffer.write(b" 200 OK\r\n")?;
	// buffer.write(b"Content-Type: text/html\n\r\n\r")?;
	buffer.push_http_content_type(ContentType::text_html)?;
	buffer.write(b"\r\n")?;

	buffer.write(b"<body><h2>Home Page</h2>")?;
	write!(buffer, "<div style=\"float: left; padding-right: 5%;\">Local Addr:<br />Peer Addr:</div><div>{:#?}<br />{:#?}</div>", client_local_addr, client_peer_addr)?;
	let filenames = GLOBALS.get_file_entry_names();
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
	return Ok(());
}

fn return_not_found(buffer: &mut StreamBuffer) -> Result<()> {
	buffer.filled = 0;
	// buffer.write(b" 404 Not Found\r\n")?;
	buffer.push_http_primary_header("HTTP/1.1", 404, "Not Found")?;
	// buffer.write(b"Content-Type: text/html\r\n\r\n")?;
	buffer.push_http_content_type(ContentType::text_html)?;
	buffer.push_http_header_tail()?;
	
	buffer.write(b"<body><h2>Route Undefined or Unavailable - 404</h2></body>")?;

	return Ok(());
}

fn serve_route_favicon(buffer: &mut StreamBuffer) -> Result<()> {
	buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(ContentType::image_x_icon)?;
	buffer.push_http_transfer_encoding(TransferEncoding::binary)?;

	#[allow(static_mut_refs)]
	let favicon_slice = GLOBALS.favicon.as_ref().unwrap();
	buffer.push_http_content_length(favicon_slice.len())?;
	buffer.write(b"\r\n")?;
	buffer.write(favicon_slice.as_ref())?;

	return Ok(());
}

fn serve_route_file(
	buffer: &mut StreamBuffer,
	uri_path: &str,
) -> Result<()> {
	let filepath = &uri_path[6..];

	let result = GLOBALS.get_file_entry_by_name(filepath);
	if let Some(file) = result {
		buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
		buffer.push_http_content_type(ContentType::text_plain)?;
		buffer.push_http_transfer_encoding(TransferEncoding::binary)?;
		buffer.push_http_content_length(file.len())?;
		buffer.push_http_content_disposition(ContentDisposition::Attachment(Some(
			filepath.split('/').rev().next().unwrap_or(filepath)
		)))?;
		buffer.write(b"\"\r\n")?;
		buffer.write(file.as_ref())?;
	}else {
		return return_not_found(buffer);
		// // buffer.write(b" 404 Not Found\r\n")?;
		// buffer.push_http_primary_header(http_version, 404, "Not Found")?;
		// // buffer.write(b"Content-Type: text/html\r\n\r\n")?;
		// buffer.push_http_content_type(ContentType::text_html)?;
		// buffer.write(b"<body><h2>404 - File Note Found</h2></body>")?;
	}

	return Ok(());
}

fn serve_playlist(buffer: &mut StreamBuffer, playlist_query: &str) -> Result<()> {

	let query_iter = playlist_query.split('&');

	let mut playlist_name_param: Option<&str> = None;
	let mut song_number_param: Option<&str> = None;

	for query in query_iter {
		if query.starts_with("playlist=") {
			if playlist_name_param.is_some() {
				return_routing_error(buffer, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				playlist_name_param = Some(&query[9..]);
			}
		}
		else if query.starts_with("song_number=") {
			if song_number_param.is_some() {
				return_routing_error(buffer, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				song_number_param = Some(&query[12..]);
			}
		}
		else {
			return_routing_error(buffer, &format!("unrecognized query parameter: {}", query));
			return Ok(());
		}
	}

	let playlist_name = match playlist_name_param {
		Some(name) => name,
		None => {
			todo!();
			// SERVE A PLAYLIST BROWSER HERE
		}
	};

	let song_number = match song_number_param {
		Some(number_string) => match number_string.parse::<u32>() {
			Ok(number) => number,
			Err(e) => todo!()
		},
		None => 0
	};

	// let playlist_names = GLOBALS.playlist_names.as_ref().unwrap().lock()
	// 	.expect("Failed to lock mutex");
	// let playlist_names = GLOBALS.playlists.as_ref().unwrap().read()
	// 	.unwrap().iter()
	// 	.map(|playlist| playlist.name);

	// let (iter, _name) = ;
	if let Some((iter, _name)) = GLOBALS.playlists.as_ref().unwrap()
		.read().unwrap()
		.iter()
		.map(|playlist| playlist.name.clone())
		.enumerate()
		.find(|(_iter, name)| name.as_ref() == playlist_name)
	{
		let playlists = GLOBALS.playlists.as_ref().unwrap().read().unwrap();

		let playlist = playlists.get(iter).unwrap();

		buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
		buffer.push_http_content_type(ContentType::text_html)?;
		// buffer.write(b"\r\n")?;
		buffer.push_http_header_tail()?;
		
		buffer.write(b"<h1>TODO add static html file serving</h1>")?;

		// buffer.write(b"<audio controls><source src="/playlist/songs?playlist={}"</audio>)
		write!(buffer,
			"<audio controls><source src=\"/playlist/songs?playlist={}&song_number={}\"></audio>",
			playlist_name, song_number
		)?;

		buffer.write(b"<ol>")?;

		// let playlist_filenames = playlist.files.filenames.clone();
		for item_name in playlist.files.filenames.iter() {
			write!(buffer, "<li>{}</li>", item_name)?;
		}

		buffer.write(b"</ol>")?;
	}else {
		return_routing_error(buffer, &format!("Unable to fetch playlist {}", playlist_name));
		// return return_not_found(buffer);
	}
	
	return Ok(());
}

fn serve_playlist_song(buffer: &mut StreamBuffer, uri_query: &str) -> Result<()> {


	let uri_query_iter = uri_query.split('&');

	let mut playlist_name_param: Option<&str> = None;
	let mut song_number_param: Option<&str> = None;

	for query in uri_query_iter {
		if query.starts_with("playlist=") {
			if playlist_name_param.is_some() {
				return_routing_error(buffer, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				playlist_name_param = Some(&query[9..]);
			}
		}
		else if query.starts_with("song_number=") {
			if song_number_param.is_some() {
				return_routing_error(buffer, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				song_number_param = Some(&query[12..]);
			}
		}
		else {
			return_routing_error(buffer, &format!("unrecognized query parameter: {}", query));
			return Ok(());
		}
	}

	let playlist_name = match playlist_name_param {
		Some(name) => name,
		None => {
			todo!();
		}
	};

	let song_number = match song_number_param {
		Some(number_string) => match number_string.parse::<u32>() {
			Ok(number) => number,
			Err(e) => todo!()
		},
		None => 0
	};


	buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(ContentType::audio_flac)?;
	buffer.push_http_transfer_encoding(TransferEncoding::binary)?;
	buffer.push_http_content_disposition(ContentDisposition::Inline)?;

	// let playlists_lock = GLOBALS.playlists.as_ref().unwrap().lock()
	// 	.expect("Failed to lock playlists");
	// let playlist_names_lock = GLOBALS.playlist_names.as_ref().unwrap().lock()
	// 	.expect("Failed to lock playlist names");

	let songmap = match GLOBALS.get_song_by_playlist_and_index(playlist_name, song_number) {
		Some(songmap) => songmap,
		None => {
			return_routing_error(buffer, &format!("Unable to find playlist name: {}", playlist_name));
			return Ok(());
		}
	};


	let file_slice: &[u8] = songmap.as_ref();

	buffer.push_http_content_length(file_slice.len())?;
	buffer.push_http_header_tail()?;

	buffer.write(file_slice)?;

	return Ok(());
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

	let _http_version = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	let mut iobuf: [u8; 4096] = unsafe{ std::mem::zeroed() };
	let mut buffer = StreamBuffer::new(&mut iobuf, &mut client);


	let mut uri_query_splitter = request_uri.split('?');

	let uri_path = match uri_query_splitter.next() {
		Some(path) => path,
		None => request_uri
	};
	let uri_query = match uri_query_splitter.next() {
		Some(query) => query,
		None => ""
	};


	let mut uri_path_iter = uri_path.split('/');
	let mut uri_path_base = uri_path_iter.next();
	if let Some("") = uri_path_base { uri_path_base = uri_path_iter.next(); }

	match uri_path_base {
		None | Some("") => serve_route_index(client_local_addr, client_peer_addr, &mut buffer)?,
		Some("favicon.ico") => serve_route_favicon(&mut buffer)?,
		Some("file") => serve_route_file(&mut buffer, uri_path)?,
		Some("playlist") => {
			// let next = uri_path_iter.next();
			match uri_path_iter.next() {
				Some("songs") => { serve_playlist_song(&mut buffer, uri_query)?; },
				Some(_) => { return_not_found(&mut buffer)?; },
				None => { serve_playlist(&mut buffer, uri_query)?; }
			}
			// if next.is_some() { return_not_found(&mut buffer)?; }
			// else { serve_playlist(&mut buffer, uri_query)?; }
		}
		_ => return_not_found(&mut buffer)?,
	}


	buffer.flush()?;
	client.shutdown(std::net::Shutdown::Both)?;

	return Ok(());
}


fn main() {
	
	if let Ok(filebytes) = std::fs::read("entries.txt") {
		let filestring = convert_c_string(&filebytes);
		// let filestring = filebytes.as_ascii().unwrap();
		for line in filestring.split('\n') {
			if let Err(_) = GLOBALS.push_file_entry(line, line) {
				println!("Failed to open entry from file: {}", line);
			}
			// if let Ok(entry_string) = std::fs::read(line) {
			// 	GLOBALS.push_file_entry(line, &convert_c_string(&entry_string));
			// }else { println!("Failed to open entry from file"); }
		}
	}else { println!("Failed to open entries file"); }

	let mut playlist_directories: Vec<Box<str>> = vec!();

	if let Ok(filebytes) = std::fs::read("playlists.txt") {
		let filestring = convert_c_string(&filebytes);
		// let filestring = filebytes.as_ascii().unwrap();
		for line in filestring.split('\n') {
			// if let Err(_) = GLOBALS.push_file_entry(line, line) {
			// 	println!("Failed to open entry from file: {}", line);
			// }
			if let Err(_) = GLOBALS.push_playlist_directory(line) {
				println!("Failed to add playlist directory to playlists");
			}else {
				playlist_directories.push(Box::from(line));
			}
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
		}else if line_str == "show" {
			for filename in GLOBALS.file_entries.as_ref().unwrap().read().unwrap().filenames.iter() {
				println!("\r-> file     {}", filename);
			}
			for playlist in GLOBALS.playlists.as_ref().unwrap().read().unwrap().iter() {
				println!("\r-> playlist {}", playlist.name);
			}
		}
		else if line_str.starts_with("add_playlist ") {
			let playlist_dir = &line_str[13..];
			if std::path::Path::is_dir(playlist_dir.as_ref()) {
				println!("\rINFO: adding playlist");
				GLOBALS.push_playlist_directory(playlist_dir).expect("Failed to push playlist to globals");
				playlist_directories.push(Box::from(playlist_dir));
			}
			else if std::path::Path::exists(playlist_dir.as_ref()) {
				println!("Error: playlist {} is not a directory", playlist_dir);
			} else {
				println!("Error: directory {} does not exist", playlist_dir);
			}
		}
		else if line_str.starts_with("add ") {
			let filename = &line_str[4..];
			// if std::fs::exists(filename).expect("Failed to check file existence") {
			if std::path::Path::is_file(filename.as_ref()) {
				println!("INFO: adding file {} to database", filename);
				// GLOBALS.push_file_entry(filename, &convert_c_string(&std::fs::read(filename).expect("Failed to read from file")));
				if let Err(e) = GLOBALS.push_file_entry( filename, filename ) {
					println!("Error: failed to map file {} | {}", filename, e);
				}
			}else if std::path::Path::is_dir(filename.as_ref()) {
				// println!("INFO: adding dirctory {} (recursively) to database", filename);
				println!("INFO: adding directories is not yet supported");
			}
			else {
				println!("ERR: unable to locate file {}", filename);
			}
		}
		buffer.clear();
	}


	let mut entry_file = std::fs::File::create("entries.txt").expect("Failed to open entries file for saving");
	for entry in GLOBALS.get_file_entry_names() {
		entry_file.write(entry.as_bytes()).expect("failed to write to file");
		entry_file.write(b"\n").expect("failed to write to file");
	}

	let mut playlist_file = std::fs::File::create("playlists.txt").expect("Failed to create/open playlists file");
	for playlist in playlist_directories {
		write!(playlist_file, "{}\n", playlist).expect("Failed to write to playlist file");
	}
	
	crossterm::terminal::disable_raw_mode().expect("Failed to exit raw mode");

	return;
}


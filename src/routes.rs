
use std::{io::{Read, Write}, str::FromStr};

use anyhow::Result;

use crate::{globals::GLOBALS, http::{make_http_request, ClosureReader, HttpHeader, ReadInto}};



fn return_routing_error(buffer: &mut crate::http::StreamBuffer, error_string: &str) {
	buffer.push_http_response_primary_header("HTTP/1.1", 400, "Routing Error").unwrap();

	buffer.begin_http_body().unwrap();
	write!(buffer, "<h2>Routing Error</h2><p>{}</p>", error_string).unwrap();
	buffer.flush().unwrap();
}

fn serve_get_index(
	client_local_addr: std::net::SocketAddr,
	client_peer_addr: std::net::SocketAddr,
	buffer: &mut crate::http::StreamBuffer
) -> Result<()> {

	// // TODO this is all very crazy and likely quite slow
	// let mut peer_files: Vec<(std::net::IpAddr, Vec<Box<str>>)> = vec!();
	// for addr in GLOBALS.read_peers().iter() {
	// 	GLOBALS.push_thread(move || {
	// 		let mut stream  = match std::net::TcpStream::connect((*addr, 8000)) {
	// 			Ok(stream) => stream,
	// 			Err(e) => {
	// 				println!("\rError: failed to connect to peer -> {}", e);
	// 				return Ok(())
	// 			}
	// 		};
	// 		#[allow(invalid_value)]
	// 		let mut output_buffer: [u8; 4096] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };

	// 		let mut stream_output_buffer = crate::http::StreamBuffer::new(&mut output_buffer[0..], &mut stream);
	// 		make_http_request(&mut stream_output_buffer, crate::http::HttpMethod::GET, "/files", &[])?;

	// 		let mut input_buffer = String::new();
	// 		stream.read_to_string(&mut input_buffer)?;

	// 		if let Some((_head, body)) = input_buffer.as_str().split_once("\r\n\r\n") {
	// 			let mut current_peer_files = vec!();
	// 			for file in body.split('\n') {
	// 				current_peer_files.push(Box::from(file));
	// 			}
	// 			peer_files.push((*addr, current_peer_files));
	// 		} else {
	// 			println!("\rError: unexpected response from peer");
	// 			continue;
	// 		}
	// 	})
	// }
	
	buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::text_html)?;
	buffer.write(b"\r\n")?;


	let index_html_file = GLOBALS.get_static_file("index.html");
	buffer.write_templated(
		index_html_file.as_ref().unwrap(),
		&["peer_addr", "local_addr", "hosted_files", "hosted_playlists"],
		&mut [
			&mut format!("{:?}", client_local_addr).as_bytes(),
			&mut format!("{:?}", client_peer_addr).as_bytes(),
			&mut iterator_reader!(
				filename, GLOBALS.read_file_entries().filenames.iter(),
				[ b"<a href=\"/file/", filename.as_bytes(), b"\">", filename.as_bytes(), b"</a><br />" ]
			),
			// &mut iterator_reader!(
			// 	peer_entry_tuple, peer_files.iter(),
			// 	[ b"<li>peer |", peer_entry_tuple.0.to_string().as_bytes(), b"| has files" ]
			// ),
			&mut iterator_reader!(
				playlist_name, GLOBALS.read_playlists().iter().map(|playlist| playlist.name.as_ref()),
				[
					b"<a href=\"/playlist?playlist=", playlist_name.as_bytes(),
					b"&song_number=0\">", playlist_name.as_bytes(), b"</a><br />"
				]
			),
		]
	)?;
	return Ok(());
}

fn return_not_found(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	buffer.clear();
	buffer.push_http_response_primary_header("HTTP/1.1", 404, "Not Found")?;
	buffer.push_http_content_type(crate::http::ContentType::text_html)?;

	
	buffer.push_http_body(b"<body><h2>Route Undefined or Unavailable - 404</h2></body>")?;

	return Ok(());
}

fn serve_get_favicon(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::image_x_icon)?;
	buffer.push_http_transfer_encoding(crate::http::TransferEncoding::binary)?;

	buffer.push_http_content_length(GLOBALS.favicon.len())?;
	buffer.write(b"\r\n")?;
	buffer.write(GLOBALS.favicon.as_ref())?;

	return Ok(());
}

fn serve_get_file(
	buffer: &mut crate::http::StreamBuffer,
	uri_path: &str,
) -> Result<()> {
	let filepath = &uri_path[6..];

	let result = GLOBALS.get_file_entry_by_name(filepath);
	if let Some(file) = result {
		buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
		buffer.push_http_content_type(crate::http::ContentType::text_plain)?;
		buffer.push_http_transfer_encoding(crate::http::TransferEncoding::binary)?;
		buffer.push_http_content_length(file.len())?;
		buffer.push_http_content_disposition(crate::http::ContentDisposition::Attachment(Some(
			filepath.split('/').rev().next().unwrap_or(filepath)
		)))?;
		buffer.write(b"\"\r\n")?;
		buffer.write(file.as_ref())?;
	}else {
		return return_not_found(buffer);
	}

	return Ok(());
}

fn serve_get_files(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::text_plain)?;
	buffer.write(b"\r\n")?;

	for entry in GLOBALS.read_file_entries().filenames.clone() {
		write!(buffer, "{}\n", entry)?;
	}

	return Ok(());
}

fn serve_playlist(buffer: &mut crate::http::StreamBuffer, playlist_query: &str) -> Result<()> {

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
			Err(e) => {
				return_routing_error(buffer, &format!("song_number parameter failed to parsde: {}", e));
				return Ok(());
			}
		},
		None => 0
	};

	if let Some((iter, _name)) = GLOBALS.read_playlists()
		.iter()
		.map(|playlist| playlist.name.clone())
		.enumerate()
		.find(|(_iter, name)| name.as_ref() == playlist_name)
	{
		let playlists = GLOBALS.read_playlists();

		let playlist = playlists.get(iter).unwrap();

		buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
		buffer.push_http_content_type(crate::http::ContentType::text_html)?;

		
		buffer.begin_http_body()?;

		
		buffer.write_templated(
			GLOBALS.get_static_file("playlist.html").unwrap().as_ref(),
			&["playlist_name", "playlist_songs"],
			&mut [
				&mut playlist_name.as_bytes(),
				&mut iterator_reader!(
					name, playlist.files.filenames.iter()
						.map(|filename| filename.split('/').rev().next().unwrap_or(filename)),
					[ b"\"", name.as_bytes(), b"\"," ]
				)
			]
		)?;
	}else {
		return_routing_error(buffer, &format!("Unable to fetch playlist {}", playlist_name));
	}
	
	return Ok(());
}

fn serve_playlist_song(buffer: &mut crate::http::StreamBuffer, uri_query: &str) -> Result<()> {


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
			return_routing_error(buffer, "unable to parse playlist name parameter");
			return Ok(());
		}
	};

	let song_number = match song_number_param {
		Some(number_string) => match number_string.parse::<u32>() {
			Ok(number) => number,
			Err(e) => {
				return_routing_error(buffer, &format!("failed to parse song_number parameter: {}", e));
				return Ok(());
			}
		},
		None => 0
	};


	buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::audio_flac)?;
	buffer.push_http_transfer_encoding(crate::http::TransferEncoding::binary)?;
	buffer.push_http_content_disposition(crate::http::ContentDisposition::Inline)?;


	let songmap = match GLOBALS.get_song_by_playlist_and_index(playlist_name, song_number) {
		Some(songmap) => songmap,
		None => {
			return_routing_error(buffer, &format!("Unable to find playlist name: {}", playlist_name));
			return Ok(());
		}
	};


	let file_slice: &[u8] = songmap.as_ref();

	buffer.push_http_content_length(file_slice.len())?;

	buffer.push_http_body(file_slice)?;

	return Ok(());
}

fn serve_get_peers(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::text_plain)?;
	buffer.write(b"\r\n")?;

	buffer.write_templated(
		b"%peer_addrs%",
		&["peer_addrs"],
		&mut [
			&mut iterator_reader!(addr, GLOBALS.read_peers().iter(), [
				addr.to_string().as_bytes(), b"\n"
			])
		]
	)?;

	return Ok(());
}

fn serve_get_peer_files(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	let mut fetch_pool = crate::ThreadPool::new();
	for peer_addr in GLOBALS.read_peers().clone().into_iter() {
		fetch_pool.spawn(move || {
			let mut stream = std::net::TcpStream::connect((peer_addr, 8000))?;
			stream.write(b"GET /files HTTP/1.1")?;

			// NOTE this may fix a problem with incomplete reads if un-comented
			std::thread::sleep(std::time::Duration::from_millis(10));

			#[allow(invalid_value)]
			let mut buffer: [u8; 4096] = unsafe{ std::mem::MaybeUninit::uninit().assume_init() };
			let buffer_filled = stream.read(&mut buffer)?;

			let (_head, body) = unsafe{ buffer[0..buffer_filled].as_ascii_unchecked() }
				.as_str().split_once("\r\n")
				.expect("Failed to split body from input from peer");

			// for file in body.split("\n") {
				
			// }
			let files = body.split("\n").map(|file| file.to_owned())
				.collect::<Vec<String>>();

			return Ok((peer_addr, files));
		});
	}

	buffer.push_http_response_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::text_html)?;
	buffer.write(b"\r\n[")?;
	buffer.flush()?;

	for peer_entry in fetch_pool.into_iter() {
		match peer_entry {
			Some(Ok((peer_addr, file_list))) => {
				for file in file_list { write!(buffer, "[\"{}\", \"{}\"]", peer_addr, file)?; }
				buffer.flush()?;
			},
			Some(Err(e)) => {
				println!("\rError: failed to fetch files from peer -> {e}");
			},
			None => { std::thread::sleep(std::time::Duration::from_millis(10)); }
		}
	}
	
	return Ok(());
}

fn serve_post_peers(
	headers: &[crate::http::HttpHeader],
	body: &[u8],
) -> Result<()> {

	let body_str = body.as_ascii().unwrap().as_str();

	for line in body_str.split('\n') {
		if let Ok(addr) = std::net::IpAddr::from_str(line) {
			GLOBALS.push_peer(addr);
		}else {
			todo!();
		}
	}

	return Ok(());
}

pub fn handle_client(mut client: std::net::TcpStream) -> Result<()> {
	let client_peer_addr = client.peer_addr()?;
	let client_local_addr = client.local_addr()?;

	let mut buffer: [u8; 4096] = unsafe{ std::mem::zeroed() };
	let mut buffer_filled = 0;

	// let bufreader = std::io::BufReader::with_capacity(4096, client);
	// let mut buffer = String::with_capacity(4096);

	buffer_filled += client.read(&mut buffer)?;
// let _ = client.read_to_string(&mut buffer)?;
// let buf_cursor = std::io::BorrowedBuf::from(unsafe{ buffer.as_bytes_mut() });
// let _ = client.read_buf(buf_cursor);
	// let buffer_string = unsafe{ buffer.as_ascii_unchecked() }.as_str();

	let (head, body) = match unsafe{ buffer.as_ascii_unchecked() }.as_str().split_once("\r\n\r\n") {
		Some(split_tuple) => split_tuple,
		None => {
			std::thread::sleep(std::time::Duration::from_millis(20));
			buffer_filled += client.read(&mut buffer[buffer_filled..])?;
			let buffer_str = unsafe{ buffer.as_ascii_unchecked() }.as_str();
			match buffer_str.split_once("\r\n\r\n") {
				Some(split_tuple) => split_tuple,
				None => {
					println!("\rWARN: failed to read body separator for request even after waiting");
					(buffer_str, "")
				}
			}
		}
	};
		// .unwrap_or((&buffer_string, ""));
	let body = body.as_bytes();

	let mut header_iterator = head.split("\r\n");
	let request_line = header_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;
	let mut request_line_iterator = request_line.split(" ");
	let request_method_string = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	let mut headers = vec!();
	let mut header = header_iterator.next();

	// TODO parse Content-Length http header so that a body can be fully downloaded
	// if it is a substantial size
	loop {
		if header.is_none() { break; }
		if header.unwrap() == "" { break; }

		if let Some((header_type, header_value)) = header.unwrap().split_once(": ") {
			if let Some(header) = HttpHeader::from_str_pair(header_type, header_value) {
				headers.push(header);
			}else { println!("\rDBG: malformed or unhandled http header -> {}", header.unwrap()); }
		}else { println!("\rDBG: received malformed http header -> {}", header.unwrap()); }

		header = header_iterator.next();
	}

	let request_method: crate::http::HttpMethod;
	let request_uri: &str;

	if request_method_string == "GET" { request_method = crate::http::HttpMethod::GET; }
	else if request_method_string == "POST" { request_method = crate::http::HttpMethod::POST; }
	else { bail!("Unhandled HTTP request type"); }


	request_uri = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	println!(
		"\rINFO: serving http request from {:?} for route {:?} {:?}",
		client_peer_addr, request_method, request_uri
	);

	let _http_version = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	let mut iobuf: [u8; 4096] = unsafe{ std::mem::zeroed() };
	let mut buffer = crate::http::StreamBuffer::new(&mut iobuf, &mut client);


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

	match request_method {
		crate::http::HttpMethod::GET => {
			match uri_path_base {
				None | Some("") => serve_get_index(client_local_addr, client_peer_addr, &mut buffer)?,
				Some("favicon.ico") => serve_get_favicon(&mut buffer)?,
				Some("file") => serve_get_file(&mut buffer, uri_path)?,
				Some("files") => serve_get_files(&mut buffer)?,
				Some("playlist") => {
					match uri_path_iter.next() {
						Some("songs") => { serve_playlist_song(&mut buffer, uri_query)?; },
						Some(_) => { return_not_found(&mut buffer)?; },
						None => { serve_playlist(&mut buffer, uri_query)?; }
					}
				},
				Some("peers") => { serve_get_peers(&mut buffer)?; },
				Some("peer_files") => { serve_get_peer_files(&mut buffer)?; }
				_ => return_not_found(&mut buffer)?,
			}
		},
		crate::http::HttpMethod::POST => {
			match uri_path_base {
				Some("peers") => { serve_post_peers(&headers, body)?; },
				_ => return_not_found(&mut buffer)?,
			}
		},
		_ => return_not_found(&mut buffer)?
	};


	buffer.flush()?;
	client.shutdown(std::net::Shutdown::Both)?;

	return Ok(());
}



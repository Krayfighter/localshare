
use std::io::{Read, Write};

use anyhow::Result;

use crate::{globals::GLOBALS, http::ClosureReader, http::ReadInto};



fn return_routing_error(buffer: &mut crate::http::StreamBuffer, error_string: &str) {
	buffer.push_http_primary_header("HTTP/1.1", 400, "Routing Error").unwrap();

	buffer.begin_http_body().unwrap();
	write!(buffer, "<h2>Routing Error</h2><p>{}</p>", error_string).unwrap();
	buffer.flush().unwrap();
}

fn serve_route_index(
	client_local_addr: std::net::SocketAddr,
	client_peer_addr: std::net::SocketAddr,
	buffer: &mut crate::http::StreamBuffer
) -> Result<()> {
	buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
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
			&mut iterator_reader!(
				playlist_name, GLOBALS.read_playlists().iter().map(|playlist| playlist.name.as_ref()),
				[
					b"<a href=\"/playlist?playlist=", playlist_name.as_bytes(),
					b"&song_number=0\">", playlist_name.as_bytes(), b"</a><br />"
				]
			)
		]
	)?;
	return Ok(());
}

fn return_not_found(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	buffer.clear();
	buffer.push_http_primary_header("HTTP/1.1", 404, "Not Found")?;
	buffer.push_http_content_type(crate::http::ContentType::text_html)?;

	
	buffer.push_http_body(b"<body><h2>Route Undefined or Unavailable - 404</h2></body>")?;

	return Ok(());
}

fn serve_route_favicon(buffer: &mut crate::http::StreamBuffer) -> Result<()> {
	buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
	buffer.push_http_content_type(crate::http::ContentType::image_x_icon)?;
	buffer.push_http_transfer_encoding(crate::http::TransferEncoding::binary)?;

	#[allow(static_mut_refs)]
	let favicon_slice = GLOBALS.favicon.as_ref().unwrap();
	buffer.push_http_content_length(favicon_slice.len())?;
	buffer.write(b"\r\n")?;
	buffer.write(favicon_slice.as_ref())?;

	return Ok(());
}

fn serve_route_file(
	buffer: &mut crate::http::StreamBuffer,
	uri_path: &str,
) -> Result<()> {
	let filepath = &uri_path[6..];

	let result = GLOBALS.get_file_entry_by_name(filepath);
	if let Some(file) = result {
		buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
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

		buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
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


	buffer.push_http_primary_header("HTTP/1.1", 200, "OK")?;
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

pub fn handle_client(mut client: std::net::TcpStream) -> Result<()> {
	let client_peer_addr = client.peer_addr()?;
	let client_local_addr = client.local_addr()?;
	// print!("\rINFO: serving http request from {:?} for route", client_peer_addr);
	let mut buffer: [u8; 4096] = unsafe{ std::mem::zeroed() };

	let request_type: crate::http::HttpRequestType;
	let request_uri: &str;

	client.read(&mut buffer)?;
	let buffer_string = unsafe{ buffer.as_ascii_unchecked() }.as_str();

	let mut header_iterator = buffer_string.split("\r\n");
	let request_line = header_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;
	let mut request_line_iterator = request_line.split(" ");
	let request_type_string = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	if request_type_string == "GET" { request_type = crate::http::HttpRequestType::GET; }
	else if request_type_string == "POST" { request_type = crate::http::HttpRequestType::POST; }
	else { bail!("Unhandled HTTP request type"); }

	// print!(" {:?}", request_type);

	request_uri = request_line_iterator.next()
		.ok_or_else(|| { anyhow!("Invalid HTTP request structure") })?;

	// println!(" {:?}", request_uri);
	println!(
		"\rINFO: serving http request from {:?} for route {:?} {:?}",
		client_peer_addr, request_type, request_uri
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

	match uri_path_base {
		None | Some("") => serve_route_index(client_local_addr, client_peer_addr, &mut buffer)?,
		Some("favicon.ico") => serve_route_favicon(&mut buffer)?,
		Some("file") => serve_route_file(&mut buffer, uri_path)?,
		Some("playlist") => {
			match uri_path_iter.next() {
				Some("songs") => { serve_playlist_song(&mut buffer, uri_query)?; },
				Some(_) => { return_not_found(&mut buffer)?; },
				None => { serve_playlist(&mut buffer, uri_query)?; }
			}
		}
		_ => return_not_found(&mut buffer)?,
	}


	buffer.flush()?;
	client.shutdown(std::net::Shutdown::Both)?;

	return Ok(());
}



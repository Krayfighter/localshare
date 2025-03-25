
use std::{io::{Read, Write}, str::FromStr};

use anyhow::Result;

use crate::{
	globals::GLOBALS,
	http::{
		ClosureReader,
		HttpHeader,
		ReadInto,
		ContentType,
		TransferEncoding,
		ContentDisposition,
	}
};



fn return_routing_error(sink: &mut dyn Write, error_string: &str) {
	let response = crate::http::HttpResponse{
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec!(),
		body: b""
	};
	
	let mut sources: &mut [&mut dyn ReadInto] = &mut [
		&mut b"<h2>Routing Error</h2><p>".as_slice(),
		&mut error_string.as_bytes(),
		&mut b"</p>".as_slice()
	];
	let sources: &mut dyn ReadInto = &mut sources;
	response.write_from_readinto(sources, sink)
		.expect("Failed to write routeing error response to writer");

	sink.flush().unwrap();
}

fn serve_get_index(
	client_local_addr: std::net::SocketAddr,
	client_peer_addr: std::net::SocketAddr,
	// buffer: &mut crate::http::StreamBuffer
	sink: &mut dyn Write
) -> Result<()> {

	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec![
			HttpHeader::ContentType(ContentType::text_html)
		],
		body: b"",
	};

	let local_addr_string = format!("{:?}", client_local_addr);
	let peer_addr_string = format!("{:?}", client_peer_addr);

	let index_html_file = GLOBALS.get_static_file("index.html")
		.ok_or(anyhow!("Failed to fetch index.html from globals"))?;
	let mut template = crate::http::BodyTemplate {
		template: index_html_file.as_ref(),
		keys: &[b"peer_addr", b"local_addr", b"hosted_files", b"hosted_playlists"],
		values: &mut [
			&mut local_addr_string.as_bytes(),
			&mut peer_addr_string.as_bytes(),
			&mut iterator_reader!(
				filename, GLOBALS.read_file_entries().filenames.iter(),
				[ b"<a href=\"/file/", filename.as_bytes(), b"\">", filename.as_bytes(), b"</a><br />" ]
			),
			&mut iterator_reader!(
				playlist_name, GLOBALS.read_playlists().iter().map(|playlist| playlist.name.as_ref()),
				[
					b"<a href=\"/playlist?playlist=", playlist_name.as_bytes(),
					b"\">", playlist_name.as_bytes(), b"</a><br />"
				]
			),
		],
	};

	response.write_from_readinto(&mut template, sink)?;
	return Ok(());
}

fn return_not_found(sink: &mut dyn Write) -> Result<()> {
	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 404,
		status_text: "Not Found",
		headers: vec![
			crate::http::HttpHeader::ContentType(crate::http::ContentType::text_html)
		],
		body: b"<body><h2>Route Undefined or Unavailable - 404</h2></body>",
	};

	
	response.write_to_sink(sink)?;

	return Ok(());
}

fn serve_get_favicon(sink: &mut dyn Write) -> Result<()> {
	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec![
			crate::http::HttpHeader::ContentType(crate::http::ContentType::image_x_icon),
			crate::http::HttpHeader::TransferEncoding(crate::http::TransferEncoding::binary),
			crate::http::HttpHeader::ContentLength(GLOBALS.favicon.len())
		],
		body: GLOBALS.favicon.as_ref(),
	};

	response.write_to_sink(sink)?;

	return Ok(());
}

fn serve_get_file(sink: &mut dyn Write, request: &crate::http::HttpRequest) -> Result<()> {
	// let filepath = &uri_path[6..];
	// let (_, filepath) = match request.route.split_once('/') {
	// 	Some(pair) => pair,
	// 	None => {
	// 		return_routing_error(sink, "no file path in route");
	// 		return Ok(());
	// 	}
	// };
	let filepath = &request.route["/file/".len()..];

	if request.query_params == "" {
		let result = GLOBALS.get_file_entry_by_name(filepath);
		if let Some(file) = result {
			let response = crate::http::HttpResponse {
				protocol_version: "HTTP/1.1",
				status_code: 200,
				status_text: "OK",
				headers: vec![
					HttpHeader::ContentType(crate::http::ContentType::text_plain),
					HttpHeader::TransferEncoding(crate::http::TransferEncoding::binary),
					HttpHeader::ContentLength(file.len()),
					HttpHeader::ContentDisposition(ContentDisposition::Attachment(Some(
						filepath.split('/').rev().next().unwrap_or(filepath)
					)))
				],
				body: file.as_ref(),
			};
			response.write_to_sink(sink)?;
		}else {
			return return_not_found(sink);
		}
	}else {
		if !request.query_params.starts_with("source=") {
			return_routing_error(sink, "query parameter should be formatted as ?sourece=xxxxx");
			return Ok(());
		}
		let (_key, value) = request.query_params.split_once("=").unwrap();
		println!("\rDBG: source param -> {}", value);
		match std::net::IpAddr::parse_ascii(value.as_bytes()) {
			Ok(addr) => {
				let mut peer_stream = std::net::TcpStream::connect((addr, 8000))?;
				peer_stream.set_nonblocking(false)?;
				{
					let mut request = request.clone();
					request.query_params = "";
					request.write_to_sink(&mut peer_stream)?;
					// wait for peer to respond
					std::thread::sleep(std::time::Duration::from_millis(100));
				}
				let mut buffer = Vec::<u8>::new();
				let response = crate::http::HttpResponse::read_blocking(&mut buffer, &mut peer_stream)?;
				response.write_to_sink(sink)?;
			},
			Err(_e) => {
				return_routing_error(sink, "failed to parse address from query parameter");
				return Ok(());
			}
		}
	}

	return Ok(());
}

fn serve_get_files(sink: &mut dyn Write) -> Result<()> {
	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec![
			HttpHeader::ContentType(ContentType::text_plain)
		],
		body: b"",
	};

	response.write_from_readinto(
		&mut ClosureReader{ source: &|dest| {
			let mut write_size = 0;
			for filename in GLOBALS.read_file_entries().filenames.clone() {
				write_size += dest.write(filename.as_bytes())?;
				write_size += dest.write(b"\n")?;
			}
			return Ok(write_size);
		}},
		sink
	)?;

	return Ok(());
}

fn serve_get_playlist(sink: &mut dyn Write, request: &crate::http::HttpRequest) -> Result<()> {
	let mut playlist_name_param: Option<&str> = None;

	for query in request.query_params.split('&') {
		if query.starts_with("playlist=") {
			if playlist_name_param.is_some() {
				return_routing_error(sink, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				playlist_name_param = Some(&query[9..]);
			}
		}
		else {
			return_routing_error(sink, &format!("unrecognized query parameter: {}", query));
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

	if let Some((iter, _name)) = GLOBALS.read_playlists()
		.iter()
		.map(|playlist| playlist.name.clone())
		.enumerate()
		.find(|(_iter, name)| name.as_ref() == playlist_name)
	{
		let playlists = GLOBALS.read_playlists();

		let playlist = playlists.get(iter).unwrap();

		let response = crate::http::HttpResponse {
			protocol_version: "HTTP/1.1",
			status_code: 200,
			status_text: "OK",
			headers: vec![
				HttpHeader::ContentType(ContentType::text_html)
			],
			body: b"",
		};

		let playlist_template = GLOBALS.get_static_file("playlist.html")
				.ok_or(anyhow!("Failed to fetch playlist.html from globals"))?;
		let mut template = crate::http::BodyTemplate {
			template: playlist_template.as_ref(),
			keys: &[b"playlist_name", b"playlist_songs"],
			values: &mut [
				&mut playlist_name.as_bytes(),
				&mut iterator_reader!(
					name, playlist.files.filenames.iter()
						.map(|filename| filename.split('/').rev().next().unwrap_or(filename)),
					[ b"\"", name.as_bytes(), b"\"," ]
				)
			],
		};
		response.write_from_readinto(&mut template, sink)?;
	}else {
		return_routing_error(sink, &format!("Unable to fetch playlist {}", playlist_name));
	}
	
	return Ok(());
}

fn serve_get_playlist_song(sink: &mut dyn Write, request: &crate::http::HttpRequest) -> Result<()> {
	let mut playlist_name_param: Option<&str> = None;
	let mut song_number_param: Option<&str> = None;

	for query in request.query_params.split('&') {
		if query.starts_with("playlist=") {
			if playlist_name_param.is_some() {
				return_routing_error(sink, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				playlist_name_param = Some(&query[9..]);
			}
		}
		else if query.starts_with("song_number=") {
			if song_number_param.is_some() {
				return_routing_error(sink, &format!("duplicate query paramter: {}", query));
				return Ok(());
			}else {
				song_number_param = Some(&query[12..]);
			}
		}
		else {
			return_routing_error(sink, &format!("unrecognized query parameter: {}", query));
			return Ok(());
		}
	}

	let playlist_name = match playlist_name_param {
		Some(name) => name,
		None => {
			return_routing_error(sink, "unable to parse playlist name parameter");
			return Ok(());
		}
	};

	let song_number = match song_number_param {
		Some(number_string) => match number_string.parse::<u32>() {
			Ok(number) => number,
			Err(e) => {
				return_routing_error(sink, &format!("failed to parse song_number parameter: {}", e));
				return Ok(());
			}
		},
		None => 0
	};


	let songmap = match GLOBALS.get_song_by_playlist_and_index(playlist_name, song_number) {
		Some(songmap) => songmap,
		None => {
			return_routing_error(sink, &format!("Unable to find playlist name: {}", playlist_name));
			return Ok(());
		}
	};

	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec![
			HttpHeader::ContentType(ContentType::audio_flac),
			HttpHeader::ContentDisposition(ContentDisposition::Inline),
			HttpHeader::ContentLength(songmap.len()),
			HttpHeader::TransferEncoding(TransferEncoding::binary),
		],
		body: songmap.as_ref(),
	};

	response.write_to_sink(sink)?;

	return Ok(());
}

fn serve_get_peers(sink: &mut dyn Write) -> Result<()> {
	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec!(),
		body: b"",
	};

	response.write_from_readinto(
		&mut ClosureReader { source: &|dest| {
			let mut write_size = 0;
			let peers = GLOBALS.read_peers();
			let peer_count = peers.len();

			for (iter, peer) in peers.iter().enumerate() {
				write_size += dest.write(peer.to_string().as_bytes())?;
				if iter < peer_count {
					write_size += dest.write(b"\n")?;
				}
			}

			return Ok(write_size);
		}},
		sink
	)?;

	return Ok(());
}

fn serve_get_peer_files(sink: &mut dyn Write) -> Result<()> {
	let mut fetch_pool = crate::ThreadPool::new();
	for peer_addr in GLOBALS.read_peers().clone().into_iter() {
		fetch_pool.spawn(move || {
			let mut stream = std::net::TcpStream::connect((peer_addr, 8000))?;
			stream.set_nonblocking(false)?;

			let request = crate::http::HttpRequest {
				protocol_version: "HTTP/1.1",
				method: crate::http::HttpMethod::GET,
				route: "/files",
				query_params: "",
				headers: vec!(),
				body: b"",
			};
			request.write_to_sink(&mut stream)?;
			stream.flush()?;

			// // NOTE this may fix a problem with incomplete reads if un-comented
			// std::thread::sleep(std::time::Duration::from_millis(10));

			let mut buffer = Vec::with_capacity(16384);
			let response = crate::http::HttpResponse::read_blocking(&mut buffer, &mut stream)?;

			let (_head, body) = match unsafe{ buffer.as_slice().as_ascii_unchecked() }
				.as_str().split_once("\r\n\r\n")
			{
				Some(pair) => pair,
				None => {
					println!(
						"\rError: failed to split NOTE: buffer contents -> {}",
						unsafe{ buffer.as_slice().as_ascii_unchecked() }.as_str()
					);
					panic!("Thread Cannot continue");
				}
			};

			let files = body.split("\n").map(|file| file.to_owned())
				.collect::<Vec<String>>();

			return Ok((peer_addr, files));
		});
	}

	let response = crate::http::HttpResponse {
		protocol_version: "HTTP/1.1",
		status_code: 200,
		status_text: "OK",
		headers: vec![
			HttpHeader::ContentType(ContentType::application_json)
		],
		body: b"",
	};

	let responses = fetch_pool.into_iter()
		.map(|peer_entry| {
			match peer_entry {
				Some(Ok(pair)) => { Some(pair) },
				Some(Err(e)) => {
					println!("\rError: failed to fetch files from peer -> {e}");
					None
				},
				None => {
					std::thread::sleep(std::time::Duration::from_millis(10));
					None
				}
			}
		})
		.filter(|entry| entry.is_some())
		.map(|entry| entry.unwrap())
		.collect::<Vec<(std::net::IpAddr, Vec<String>)>>();

	response.write_from_readinto(
		&mut ClosureReader{ source: &mut move |dest| {
			let mut write_size = 0;
			write_size += dest.write(b"[")?;
			for resp in responses.iter() {
				let mut filename_iter = resp.1.iter();
				// for filename in filename_iter {
				let mut filename = filename_iter.next();
				while filename.is_some() {
					write_size += dest.write(b"[\"")?;
					write_size += dest.write(resp.0.to_string().as_bytes())?;
					write_size += dest.write(b"\",\"")?;
					write_size += dest.write(filename.unwrap().as_bytes())?;
					write_size += dest.write(b"\"]")?;
					filename = filename_iter.next();
					if filename.is_some() {
						if filename.unwrap().as_str() == "" {
							break;
						}else {
							write_size += dest.write(b",")?;
						}
					}
				}
			}
			write_size += dest.write(b"]")?;

			return Ok(write_size);
		}},
		sink
	)?;

	sink.flush()?;
	
	return Ok(());
}


// TODO update this function
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
	client.set_nonblocking(false)?;
	let client_peer_addr = client.peer_addr()?;
	let client_local_addr = client.local_addr()?;

// 	// TODO parse Content-Length http header so that a body can be fully downloaded
	// client.set_nonblocking(true)?;
	let mut request_buffer = Vec::<u8>::new();
	let request = crate::http::HttpRequest::read_blocking(&mut request_buffer, &mut client)?;

	println!(
		"\rINFO: serving request {} {} on version {}",
		request.method.as_str(),
		request.route,
		request.protocol_version
	);

	let mut path_iter = request.route.split("/");
	let _ = path_iter.next();
	let path_base = path_iter.next()
		.unwrap_or(request.route);

	let mut buffer_backing: [u8; 16384] = unsafe{ std::mem::zeroed() };
	let mut buffer = crate::http::StreamBuffer::new(&mut buffer_backing, &mut client);

	match request.method {
		crate::http::HttpMethod::GET => {
			match path_base {
				"/" | "" => serve_get_index(client_local_addr, client_peer_addr, &mut buffer)?,
				"favicon.ico" => serve_get_favicon(&mut buffer)?,
				"file" => serve_get_file(&mut buffer, &request)?,
				"files" => serve_get_files(&mut buffer)?,
				"playlist" => {
					match path_iter.next() {
						Some("songs") => { serve_get_playlist_song(&mut buffer, &request)?; },
						Some(_) => { return_not_found(&mut buffer)?; },
						None => { serve_get_playlist(&mut buffer, &request)?; }
					}
				},
				"peers" => { serve_get_peers(&mut buffer)?; },
				"peer_files" => { serve_get_peer_files(&mut buffer)?; }
				_ => return_not_found(&mut buffer)?,
			}
		},
		crate::http::HttpMethod::POST => {
			match path_base {
				"peers" => { serve_post_peers(&request.headers, &request.body)?; },
				_ => return_not_found(&mut buffer)?,
			}
		},
		_ => return_not_found(&mut buffer)?
	};


	buffer.flush()?;
	client.shutdown(std::net::Shutdown::Both)?;

	return Ok(());
}


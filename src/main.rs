#![feature(ascii_char)]
#![feature(trait_alias)]
#![feature(unboxed_closures)]
#![feature(fn_traits)]

use std::io::Write;


#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate crossterm;


#[macro_use]
mod http;
mod routes;
mod globals;


use globals::GLOBALS;




fn main() {
	

	let listener = std::net::TcpListener::bind(
		std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
			std::net::Ipv4Addr::UNSPECIFIED,
			8000
		))
	).expect("Failed to bind tcp listener to localhost");

	println!("INFO: serving at local addr: {:?}", listener.local_addr());


	let _listener_thread_handle = std::thread::spawn(move || {
		let mut threads = Vec::<std::thread::JoinHandle<()>>::new();
		listener.set_nonblocking(true).expect("Failed to set listener socket to nonblocking");

		loop {
			if let Ok((stream, _addr)) = listener.accept() {
				threads.push(std::thread::spawn(move || { let _ = routes::handle_client(stream); } ))
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

	// let peer_listener = std::net::TcpListener::bind(
	// 	std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
	// 		std::net::Ipv4Addr::UNSPECIFIED, 12555
	// 	))
	// ).expect("Failed to bind tcp listener to 12555");

	// let _peer_listener_thread_handle = std::thread::spawn(move || {
	// 	let mut thread = Vec::<std::thread::JoinHandle<()>>::new();
	// 	peer_listener.set_nonblocking(true);

	// 	loop {
	// 		if let Ok((stream, addr)) = peer_listener.accept() {
	// 			println!("\rINFO: incoming peer at address {:?}", addr);
	// 		}
	// 		else {
	// 			let mut index = 0;
	// 			todo!()
	// 			// while index < thread
	// 		}
	// 	}
	// })



	crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");


	let mut stdout = std::io::stdout();
	let mut buffer = String::new();
	'user_mainloop: loop {

		'input: loop {
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
		let line_str = &buffer.as_str()[0..];

		if line_str == "quit" { break; }
		else if line_str == "clear" {
			queue!(stdout,
				crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
				crossterm::cursor::MoveTo(0, 0)
			).expect("Failed to queue to stdout");
		}else if line_str == "help" {
			println!("\
\rLocalShare - sharing files locally
\r----------------------------------
\r
\rquit / Ctrl-C               - quit the program / clear line then quit program
\rCtrl-W                      - clear line
\rshow                        - show the currently hosted files and playlists
\radd <file_path>             - add a file to the host list
\radd_playlist <playlist_dir> - add playlist_dir to playlists"
			);
		}else if line_str == "show" {
			for filename in GLOBALS.read_file_entries().filenames.iter() {
				println!("\r-> file     {}", filename);
			}
			for playlist in GLOBALS.read_playlists().iter() {
				println!("\r-> playlist {}", playlist.name);
			}
		}
		else if line_str.starts_with("add_playlist ") {
			let playlist_dir = &line_str[13..];
			if std::path::Path::is_dir(playlist_dir.as_ref()) {
				println!("\rINFO: adding playlist");
				GLOBALS.push_playlist_directory(playlist_dir).expect("Failed to push playlist to globals");
			}
			else if std::path::Path::exists(playlist_dir.as_ref()) {
				println!("Error: playlist {} is not a directory", playlist_dir);
			} else {
				println!("Error: directory {} does not exist", playlist_dir);
			}
		}
		else if line_str.starts_with("add ") {
			let filename = &line_str[4..];
			if std::path::Path::is_file(filename.as_ref()) {
				println!("INFO: adding file {} to database", filename);
				if let Err(e) = GLOBALS.push_file_entry( filename, filename ) {
					println!("Error: failed to map file {} | {}", filename, e);
				}
			}else if std::path::Path::is_dir(filename.as_ref()) {
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
	for playlist_dir in GLOBALS.read_playlists().iter()
		.map(|playlist| playlist.directory.clone()) {
		write!(playlist_file, "{}\n", playlist_dir).expect("Failed to write to playlist file");
	}
	
	crossterm::terminal::disable_raw_mode().expect("Failed to exit raw mode");

	return;
}


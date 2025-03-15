#![feature(ascii_char)]
#![feature(trait_alias)]
#![feature(unboxed_closures)]
#![feature(fn_traits)]
#![feature(generic_arg_infer)]

// TODO remove file serving duplcates in Globals::add_file

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


struct CommandTokenIter<'a> {
	source: &'a str,
	index: usize
}

impl<'a> CommandTokenIter<'a> {
	pub fn new(source: &'a str) -> Self {
		return Self { source, index: 0 };
	}
}

impl<'a> Iterator for CommandTokenIter<'a> {
	type Item = &'a str;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.source.len() { return None; }
		if self.source.as_bytes()[self.index] == b' ' {
			self.index += 1;
			return Self::next(self);
		}

		let mut token_end = self.index;
		let mut inside_quotes = false;

		while token_end < self.source.len() {
			if self.source.as_bytes()[token_end] == b' ' && !inside_quotes {
				let token_start = self.index;
				self.index = token_end;
				return Some(&self.source[token_start..self.index]);
			}else if self.source.as_bytes()[token_end] == b'\"' {
				if inside_quotes {
					let token_start = self.index + 1;
					self.index = token_end + 1;
					return Some(&self.source[token_start..token_end]);
				}
				else if self.index != token_end {
					let token_start = self.index;
					self.index = token_end;
					return Some(&self.source[token_start..(token_end-1)]);
				}
				else {
					inside_quotes = true;
				}
			}
			token_end += 1;
		}
		let mut token_start = self.index;
		if self.source.as_bytes()[token_start] == b'\"' { token_start += 1; };
		self.index = token_end;
		return Some(&self.source[token_start..]);
	}
}


fn main() {
	

	let listener = std::net::TcpListener::bind(
		std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
			std::net::Ipv4Addr::UNSPECIFIED,
			8000
		))
	).expect("Failed to bind tcp listener to localhost");

	println!("\rINFO: serving at local addr: {:?}", listener.local_addr());


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


	crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");


	let mut stdout = std::io::stdout();
	let mut buffer = String::new();
	'user_mainloop: loop {
		buffer.clear();
		
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

		let mut token_iterator = CommandTokenIter::new(line_str).into_iter();


		match token_iterator.next() {
			Some("quit") => {
				if let Some(_) = token_iterator.next() {
					println!("\rError: quit does not take any arguments");
				}else {
					break;
				}
			},
			Some("clear") => {
				if let Some(_) = token_iterator.next() {
					println!("\rError: clear does not take any arguments");
				}else {
					queue!(stdout,
						crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
						crossterm::cursor::MoveTo(0, 0)
					).expect("Failed to queue to stdout");
				}
			},
			Some("help") => {
				if let Some(_) = token_iterator.next() {
					println!("\rWARN: help does not currently process arguments");
				}
				println!("\
	\rLocalShare - sharing files locally
	\r----------------------------------
	\r
	\rquit / Ctrl-C               - quit the program / clear line then quit program
	\rCtrl-W                      - clear line
	\rshow                        - show the currently hosted files and playlists
	\radd <file_path>             - add a file to the host list
	\radd_playlist <playlist_dir> - add playlist_dir to playlists\
				");
			},
			Some("show") => {
				match token_iterator.next() {
					Some("files") => {
						if let Some(_) = token_iterator.next() { println!("\rToo many args to \"show\""); }
						else {
							for filename in GLOBALS.read_file_entries().filenames.iter() {
								println!("\r-> file {}", filename);
							}
						}
					},
					Some("playlists") => {
						if let Some(_) = token_iterator.next() { println!("\rToo many args to \"show\""); }
						else {
							for playlist in GLOBALS.read_playlists().iter() {
								println!("\r-> playlist {}", playlist.name);
							}
						}
					},
					Some(arg) => {
						println!("\rError: unrecognized argument to \"show\": {}", arg);
					},
					None => {
						for filename in GLOBALS.read_file_entries().filenames.iter() {
							println!("\r-> file     {}", filename);
						}
						for playlist in GLOBALS.read_playlists().iter() {
							println!("\r-> playlist {}", playlist.name);
						}
					}
				}
			},
			Some("add_playlist") => {
				match token_iterator.next() {
					Some(playlist_dir) => {
						if let Some(_) = token_iterator.next() {
							println!("\rError: too many argument to add_playlist");
							continue;
						}
						if std::path::Path::is_dir(playlist_dir.as_ref()) {
							println!("\rINFO: adding playlist");
							GLOBALS.push_playlist_directory(playlist_dir).expect("Failed to push playlist to globals");
						}
						else if std::path::Path::exists(playlist_dir.as_ref()) {
							println!("\rError: playlist {} is not a directory", playlist_dir);
						} else {
							println!("\rError: directory {} does not exist", playlist_dir);
						}
					},
					None => {
						println!("\rError: add_playlist requires an argument (playlist directory)");
					}
				}
			},
			Some("add") => {
				// match token_iterator.next() {
				// 	Some (filename) => {
				// 		if std::path::Path::is_file(filename.as_ref()) {
				// 			println!("\rINFO: adding file {} to database", filename);
				// 			if let Err(e) = GLOBALS.push_file_entry( filename, filename ) {
				// 				println!("\rError: failed to map file {} | {}", filename, e);
				// 			}
				// 		}else if std::path::Path::is_dir(filename.as_ref()) {
				// 			println!("\rINFO: adding directories is not yet supported");
				// 		}
				// 		else {
				// 			println!("\rError: unable to locate file {}", filename);
				// 		}
				// 	},
				// 	None => todo!(),
				// }
				for filename in token_iterator {
					if std::path::Path::is_file(filename.as_ref()) {
						println!("\rINFO: adding file {} to database", filename);
						if let Err(e) = GLOBALS.push_file_entry( filename, filename ) {
							println!("\rError: failed to map file {} | {}", filename, e);
						}
					}else if std::path::Path::is_dir(filename.as_ref()) {
						println!("\rINFO: adding directories is not yet supported");
					}
					else {
						println!("\rError: unable to locate file {}", filename);
					}
				}
			},
			Some("download_playlist") => todo!(),
			Some(_) => { println!("\rError: unrecognized command"); },
			None => continue 'user_mainloop,
		}
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

#[cfg(test)]
mod tests {
	#[test]
	fn test_command_iterator() {
		let command_strings = [
			"add this",
			"do \"all of these\" things",
			"remove -r \"these things",
			"quit   this",
			"exit all    ",
		];
		let command_token_strings: [&[&str]; _] = [
			&["add", "this"],
			&["do", "all of these", "things"],
			&["remove", "-r", "these things"],
			&["quit", "this"],
			&["exit", "all"],
		];

		for (command_iter, string) in command_strings.into_iter().enumerate() {
			for (iter, token) in super::CommandTokenIter::new(string)
				.into_iter()
				.enumerate()
			{
				assert_eq!(token, command_token_strings[command_iter][iter]);
			}
		}
	}
}


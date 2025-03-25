#![feature(ascii_char)]
#![feature(generic_arg_infer)]
#![feature(addr_parse_ascii)]

// TODO remove file serving duplcates in Globals::add_file

use std::{io::Write, str::FromStr, sync::Arc};


#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate crossterm;


#[macro_use]
mod http;
mod routes;
mod globals;


use globals::GLOBALS;
use anyhow::Result;
use routes::handle_client;


// NOTE this could be done faster maybe with some SIMD or
// clever optimizations
pub fn split_slice_uninclusive<'a, T: PartialEq + std::fmt::Debug>(
	source: &'a [T], subslice: &[T]
) -> Option<(&'a [T], &'a [T])> {
	if subslice.len() > source.len() { return None; }
	if subslice.len() == source.len() {
		if source == subslice { return Some((&[], &[])); }
		else { return None; }
	}

	for index in 0..(source.len()-subslice.len()) {
		assert_eq!(source[index..(index+subslice.len())].len(), subslice.len());
		if &source[index..(index+subslice.len())] == subslice {
			return Some((
				&source[..index],
				&source[index+subslice.len()..]
			));
		}
	}


	return None;
}

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


struct ThreadPool<T> {
	threads: Vec<std::thread::JoinHandle<Result<T>>>
}

impl<T: Send + Sync + 'static + Sized> ThreadPool<T> {
	pub fn new() -> Self { return Self{ threads: vec!() } }
	pub fn spawn<F: FnOnce() -> Result<T> + Send + 'static>(&mut self, closure: F) {
		let new_thread_handle = std::thread::spawn(closure);
		self.threads.push(new_thread_handle);
	}
	pub fn clean_threads(&mut self) {
		let mut index = 0;
		while index < self.threads.len() {
			if unsafe{ self.threads.get_unchecked(index) }.is_finished() {
				let thread = self.threads.swap_remove(index);
				match thread.join() {
					Err(e) => { println!("\rERROR: thread pool failed to join thread that supposedly finished -> {:?}", e) },
					Ok(Err(e)) => { println!("\rError: thread pool joined thread that encountered a critical error -> {e}") },
					_ => {}
				};
				continue;
			}
			index += 1;
		}
	}
}

impl<T> Iterator for ThreadPool<T> {
    type Item = Option<Result<T>>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.threads.len() == 0 { return None; }
		for (index, handle) in self.threads.iter().enumerate() {
			if handle.is_finished() {
				return Some(Some(
					self.threads.swap_remove(index).join().expect("Failed to join thread from pool")
				));
			}
		}
		return Some(None);
	}
}


fn main() {

	let _thread_cleaner_join_handle = std::thread::spawn(|| {
		loop {
			GLOBALS.thread_pool.lock().expect("Failed to lock global thread pool").clean_threads();
			std::thread::sleep(std::time::Duration::from_millis(100));
		}
	});

	let listener = std::net::TcpListener::bind(
		std::net::SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, 8000)
	).expect("Failed to bind tcp listener to unspecified address at port 8000");

	println!("\rINFO: serving at local addr: {:?}", listener.local_addr());

	GLOBALS.push_thread(move || {
		listener.set_nonblocking(true).expect("Failed to set tcp socket to nonblocking");
		loop {
			if let Ok((stream, _addr)) = listener.accept() {
				stream.set_nonblocking(true).expect("Failed to set stream to nonblocking mode");
				GLOBALS.push_thread(move || {
					match routes::handle_client(stream) {
						Ok(()) => {},
						Err(e) => {
							println!("\rError:  client handler returned an error -> {e}");
						}
					};
					return Ok(());
				});
			}else {
				std::thread::sleep(std::time::Duration::from_millis(10));
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
	\radd_playlist <playlist_dir> - add playlist_dir to playlists
	\rdownload_playlist <name> <playlist url> [audio format]
	\r                            - download a playlist with default audio format being flac\
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
						for peer in GLOBALS.read_peers().iter() {
							println!("\r-> peer {:?}", peer);
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
			Some("add_peer") => {
				let peer_addr = match token_iterator.next() {
					Some(addr_string) => {
						if let Some(_) = token_iterator.next() {
							println!("\rError: too many arguments to add_peer");
							continue;
						}
						match std::net::IpAddr::from_str(addr_string) {
							Ok(addr) => addr,
							Err(e) => {
								println!("\rError: failed to parse address of peer -> {}", e);
								continue;
							}
						}
					},
					None => {
						println!("\rError: add_peer expect an address");
						continue;
					}
				};
				GLOBALS.push_peer(peer_addr);
			}
			Some("download_playlist") => {
				let playlist_name = match token_iterator.next() {
					Some(name) => name,
					None => {
						println!("\rError: \"download_playlist\" requires a name argument and then a url argument");
						continue;
					}
				};
				let playlist_url = match token_iterator.next() {
					Some(token) => token,
					None => {
						println!("\rError: \"download_playlist\" requires a name argument and then a url argument");
						continue;
					}
				};
				let audio_format = token_iterator.next().unwrap_or("flac");

				let mut playlist_dirname = match std::env::current_dir() {
					Ok(dir) => dir,
					Err(e) => {
						println!("\rError: unable to get current operating directory from environment -> {}", e);
						continue;
					}
				};
				playlist_dirname.push("playlists");
				match std::fs::exists(&playlist_dirname) {
					Ok(true) => {},
					_ => {
						if let Err(e) = std::fs::DirBuilder::new().create(&playlist_dirname) {
							println!("\rError: failed to create playlists directory -> {}", e);
							continue;
						}
					}
				};
				playlist_dirname.push(playlist_name);

				// if let Ok(true) = std::fs::exists(playlist_dirname) {}
				match std::fs::exists(&playlist_dirname) {
					Ok(true) => {},
					_ => {
						if let Err(e) = std::fs::DirBuilder::new().create(&playlist_dirname) {
							println!("\rError: failed to create directory (non-recursive) from playlist name -> {}", e);
							continue;
						}
					}
				};

				let mut command = std::process::Command::new("yt-dlp");
				command.arg(playlist_url)
					.arg("-x")
					.arg("--audio-format")
					.arg(audio_format)
					.arg("--audio-quality")
					.arg("0"); // in ffmpeg 0 -> highest quality, 10 -> lowest

				command.current_dir(&playlist_dirname);
				// command.stdout(std::process::Stdio::inherit());
				// command.stderr(std::process::Stdio::inherit())

				let mut subproc = match command.spawn() {
					Ok(child) => child,
					Err(e) => {
						println!("\rError: failed to spawn yt-dlp downloader backend -> {}", e);
						continue;
					}
				};

				crossterm::terminal::disable_raw_mode().unwrap();

				let exit_status = match subproc.wait() {
					Ok(status) => status,
					Err(e) => {
						println!("\rError: failed to wait for command -> {}", e);
						continue;
					}
				};

				crossterm::terminal::enable_raw_mode().unwrap();

				if exit_status.success() {
					println!("\ryt-dlp exited successfully with code 0");
				}else {
					println!("\rError: yt-dlp failed -> {}", exit_status);
				}

				;
				if let Err(e) = GLOBALS.push_playlist_directory(
					playlist_dirname.as_os_str().to_str().expect("failed to convert OsStr to str")
				) {
					println!("Error: failed to add newly downloaded playlist to playlist database -> {}", e);
				}
			},
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

	#[test]
	fn test_split_slice_uninclusive() {
		let sources: &[&[u8]] = &[
			b"first second third",
			b"GET /files HTTP/1.1\r\nContent-Type: text/plain",
			b"first second third",
			b"This longer string",
		];
		let splitters: &[&[u8]] = &[
			b" ",
			b"\r\n",
			b" second ",
			b"first",
		];
		let results: &[Option<(&[u8], &[u8])>] = &[
			Some(( b"first", b"second third" )),
			Some(( b"GET /files HTTP/1.1", b"Content-Type: text/plain")),
			Some(( b"first", b"third" )),
			None
		];

		// assert_eq!(
		// 	crate::split_slice_uninclusive(
		// 		b"first second third",
		// 		b" ",
		// 	),
		// 	Some((&b"first"[..], &b"second third"[..]))
		// )
		for index in 0..sources.len() {
			assert_eq!(
				crate::split_slice_uninclusive(
					sources[index],
					splitters[index],
				),
				results[index]
			)
		}
	}
}


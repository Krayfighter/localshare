
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};

use anyhow::Result;


pub struct FileDatabase {
	pub filenames: Vec<Arc<str>>,
	pub file_contents: Vec<Arc<memmap2::Mmap>>
}

impl FileDatabase {
	pub fn new() -> Self {
		return Self {
			filenames: Vec::new(),
			file_contents: Vec::new()
		};
	}
	pub fn add_file(&mut self, filename: &str) -> Result<()> {
		let file = std::fs::File::open(filename);
		if let Err(_) = file { bail!("Failed to open file: {}", filename); }
		let file = file.unwrap();

		let filemap = unsafe{ memmap2::Mmap::map(&file) };
		if let Err(_) = filemap { bail!("Unable to map file to memory: {}", filename); }
		let filemap = filemap.unwrap();

		self.push_entry(Arc::from(filename), Arc::new(filemap));
		return Ok(());
	}
	pub fn add_directory_nonrecursive(&mut self, dirname: &str) -> Result<()> {
		for direntry in std::fs::read_dir(dirname)?.into_iter()
			.filter(|entry| entry.is_ok())
			.map(|entry| entry.unwrap())
			.filter(|entry| {
				if let Ok(filetype) = entry.file_type() {
					filetype.is_file()
				}else {
					false
				}
			})
		{
			if let Err(_) = self.add_file(
				direntry.path().as_os_str().to_str()
					.expect("Failed to convert C string to utf-8")
			) {
				// TODO maybe add messages here
			}
		}
		return Ok(());
	}
	pub fn push_entry(&mut self, filename: Arc<str>, filemap: Arc<memmap2::Mmap>) {
		self.filenames.push(Arc::from(filename));
		self.file_contents.push(Arc::from(filemap));
	}
}

pub struct Playlist {
	pub directory: Arc<str>,
	pub name: Arc<str>,
	pub files: FileDatabase,
}

impl Playlist {
	pub fn from_directory(playlist_dir: &str) -> Result<Self> {
		let mut playlist_files = FileDatabase::new();
		playlist_files.add_directory_nonrecursive(playlist_dir)?;

		let playlist_name = playlist_dir.split('/').rev().next()
			.unwrap_or(playlist_dir);

		return Ok(Playlist{ directory: Arc::from(playlist_dir), name: Arc::from(playlist_name), files: playlist_files });
	}
}

pub struct Globals {
	file_entries: RwLock<FileDatabase>,
	playlists: RwLock<Vec<Playlist>>,
	peers: RwLock<Vec<std::net::IpAddr>>,
	pub thread_pool:  Mutex<crate::ThreadPool<()>>,
	pub static_files: FileDatabase,
	pub favicon: memmap2::Mmap,
}

impl Globals {
	pub fn read_file_entries(&self) -> RwLockReadGuard<FileDatabase> {
		return self.file_entries
			.read().expect("Failed to get read guard from file entries RwLock");
	}

	pub fn read_playlists(&self) -> RwLockReadGuard<Vec<Playlist>> {
		return self.playlists.read()
			.expect("Failed to get read guard from playlists RwLock");
	}

	pub fn read_peers(&self) -> RwLockReadGuard<Vec<std::net::IpAddr>> {
		return self.peers.read().expect("Failed to lock global peers for reading");
	}
	
	pub fn get_file_entry_names(&self) -> Vec<Arc<str>> {
		// let entries = self.file_entries;
		let filenames = self.file_entries.read().unwrap().filenames.clone();

		return filenames;
	}

	pub fn get_file_entry_by_name(&self, name: &str) -> Option<Arc<memmap2::Mmap>> {
		let entries = self.file_entries.read().unwrap();

		let (index, _filename) = match entries.filenames
			.iter()
			.enumerate()
			.find(|(_iter, filename)| filename.as_ref() == name) {
			Some(pair) => pair,
			None => return None
		};

		return entries.file_contents.get(index).cloned();
	}

	pub fn push_file_entry<P: AsRef<std::path::Path>>(&self, name: &str, fpath: P) -> Result<()> {

		let file = std::fs::File::open(fpath.as_ref())?;
		let filemap = unsafe{ memmap2::Mmap::map(&file) }?;

		let mut entries = self.file_entries.write().unwrap();

		entries.filenames.push(Arc::from(name));
		entries.file_contents.push(Arc::from(filemap));

		return Ok(());
	}

	pub fn push_playlist_directory(&self, dirname: &str) -> Result<()> {
		
		let playlist_name = dirname.split('/').rev().next()
			.unwrap_or(dirname);

		let mut playlist = FileDatabase::new();
		
		for entry in std::fs::read_dir(dirname)? {
			if let Ok(entry) = entry {
				if entry.file_type()?.is_file() {
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

		let mut playlists = self.playlists.write().unwrap();


		playlists.push(Playlist{ directory: Arc::from(dirname), name: Arc::from(playlist_name), files: playlist });

		return Ok(());
	}

	pub fn get_song_by_playlist_and_index(&self, playlist_name: &str, song_number: u32) -> Option<Arc<memmap2::Mmap>> {
		let playlists = self.playlists.read().unwrap();

		let index = match playlists.iter()
			.map(|playlist| playlist.name.clone())
			.enumerate()
			.find(|(_iter, pname)| pname.as_ref() == playlist_name)
		{
			Some((index, _pname)) => index,
			None => return None
		};

		let playlist = match playlists.get(index) {
			Some(playlist) => playlist,
			None => return None
		};

		let songmap = playlist.files.file_contents.get(song_number as usize);

		return songmap.cloned();
	}

	pub fn get_static_file(&self, filename: &str) -> Option<Arc<memmap2::Mmap>> {

		let index = match self.static_files
			.filenames.iter()
			.enumerate()
			.find(|(_iter, fpath)| fpath.ends_with(filename))
		{
			Some((index, _fname)) => index,
			None => return None
		};

		return self.static_files.file_contents.get(index).cloned();
	}

	pub fn push_peer(&self, peer: std::net::IpAddr) {
		self.peers.write().expect("failed to lock peers for writing").push(peer);
	}

	pub fn push_thread<T: FnOnce() -> Result<()> + Send + 'static>(&self, closure: T) {
		self.thread_pool.lock().expect("Failed to lock global thread pool").spawn(closure);
	}
}

pub static GLOBALS: std::sync::LazyLock<Globals> = std::sync::LazyLock::new(|| {
	let favicon_file = std::fs::File::open("favicon.ico").expect("Failed to open favicon.ico");
	let favicon = unsafe{ memmap2::Mmap::map(&favicon_file) }.expect("Failed to map favicon.ico into memory");

	let file_entries = match std::fs::read("entries.txt") {
		Ok(filestring) => {
			let mut file_entries = FileDatabase::new();
			for line in unsafe{ filestring.as_ascii_unchecked() }.as_str().split('\n') {
				if line == "" { continue; }
				if let Err(e) = file_entries.add_file(line) {
					println!("File mapping failed -> {}", e);
					continue;
				}
			}
			file_entries
		},
		Err(e) => {
			println!("WARN: Failed to open entries file");
			FileDatabase::new()
		}
	};

	let playlists = match std::fs::read("playlists.txt") {
		Ok(filestring) => {
			let mut playlists = Vec::<Playlist>::new();
			for line in unsafe{ filestring.as_ascii_unchecked() }.as_str().split('\n') {
				if line == "" { continue; }
				match Playlist::from_directory(line) {
					Ok(playlist) => playlists.push(playlist),
					Err(e) => println!("WARN: unable to create playlist from {} -> {}", line, e)
				}
			}
			playlists
		},
		Err(_) => {
			println!("WARN: Failed to open playlists file");
			Vec::<Playlist>::new()
		}
	};

	let mut static_files = FileDatabase::new();
	static_files.add_directory_nonrecursive("./static")
		.expect("Failed to map static files directory into memory");
	
	return Globals {
		file_entries: RwLock::new(file_entries),
		playlists: RwLock::new(playlists),
		peers: RwLock::new(vec![
			std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 12, 182)),
		]),
		thread_pool: Mutex::new(crate::ThreadPool::new()),
		static_files,
		favicon
	};
});


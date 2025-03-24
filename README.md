
# LocalShare

local filesharing http server with a web client

## Dependencies

rust nightly(and Cargo)

### Rust Nightly Setup

NOTE: if rust was installed in any way other than rustup
(the standard way encouraged by the rust foundation), then
that package must be uninstalled for this to work properly

```
  # install rustup - see https://www.rust-lang.org/tools/install
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

```
  # install the nightly compiler toolchain
  rustup toolchain add nightly
```

this should also set nightly as your default toolchain if one
was not already install, however, the nightly toolchain can be
set to default with

```rustup default nightly```

### Optional Dependencies

yt-dlp (for downloading playlists from internet services)

## Building

optimized release build ```cargo build -r``` and the
executable generated will be at ```target/release/localshare```

debug build and run
```cargo run```

## Use

while running, localshare hosts its web client at both localhost:8000
and the port 8000 of your local (probably NAT'ed) ip address e.g.
192.168.1.63:8000

### CLI

The localshare command line lets you interact with the backend of
localshare like adding files or playlists.


#### Hot-Keys

```Ctrl-C``` - if the line is not empty, clear it, if it is, close localshare (gracefully)<br />
```Ctrl-W``` - clear the current line

#### Commands

```quit```                     - quit localshare gracefully<br />
```show [files/playlists]```   - show the currently hosted files and/or playlists<br />
```add [<file path>, ...]```   - add a file(s) to the hosted files list<br />
```add_playlist <directory>``` - add a directory full of music files to the playlist list<br />
```download_playlist <name> <playlist url> [audio format]```
                         - download a playlist (requires yt-dlp to be in $PATH) default audio format is flac<br />
```clear```                    - clear the screen



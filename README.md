
# LocalShare

local filesharing http server with a web client

## Dependencies

rust (and Cargo)

### Optional Dependencies

yt-dlp (for downloading playlists from internet services)

## Building

optimized release build
```cargo build -r```

debug build and run
```cargo run```

## Use

while running localshare hosts its web client at both localhost:8000
and the port 8000 of your local (probably NAT'ed) ip address e.g.
192.168.1.63:8000

### CLI

The localshare command line lets you interact with the backend of
localshare like adding files for playlists.


#### Hot-Keys

```Ctrl-C``` - if the line is not empty, clear it, if it is, close localshare (gracefully)<br />
```Ctrl-W``` - clear the current line

#### Commands

```quit```                     - quit localshare gracefully<br />
```show```                     - show the currently hosted files and playlists<br />
```add [<file path>, ...]```    - add a file(s) to the hosted files list<br />
```add_playlist <directory>``` - add a directory full of music files to the playlist list<br />
```download_playlist <name> <playlist url> [audio format]```
                         - download a playlist (requires yt-dlp to be in $PATH) default audio format is flac<br />
```clear```                    - clear the screen



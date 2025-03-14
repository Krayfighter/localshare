
# LocalShare

local filesharing http server with a web client

## dependencies

rust (and Cargo)

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

the localshare CLI has several command like ```show``` ```help``` and
```quit```. The ```help``` command shows a better catalogue of available
localshare commands



# rust_hyper

List web-app in Rust to monitor output from commands on a Raspberry Pi.  Mostly an exercise to learn rust.

Using [hyper](https://crates.io/crates/hyper), [futures_cpupool](https://crates.io/crates/futures-cpupool), [serde](https://crates.io/crates/serde), [horrorshow](https://crates.io/crates/horrorshow).

## Packages

- config - configuration types read from yml by serde_yaml
- logging - setup async logging
- main - main application - read configuration file, create route configuration, start http server
- server - http server
- utils - utilities
- handlers/command - http handler to execute a command and convert output from the command to html
- handlers/index - http handler to display index page
- handlers/not_found - http handler for unknown route
- handlers/static_file - http handler to return a static file

## Threading

### Handler Threads

1 to N handler threads each run a Tokio Core event loop and a Hyper HTTP server.  

Handler threads accept new incoming connections and do all network I/O.  This is all asynchronous thanks to Tokio and Hyper.

Each handler thread binds a listening TCP socket to accept incoming connections using SO_REUSEPORT for load balancing.

### Worker Threads

Pool of 1 to N worker threads exist in a futures_cpupool::CpuPool.  

Request handlers can request that they are invoked in a worker thread by returning true for use_worker_threadpool().

Using worker threads is done for request handers that do file I/O (handlers/static_file) or execute and await the results of commands (handlers/command).  This keeps handler threads free to do network I/O and handle new incoming connections.

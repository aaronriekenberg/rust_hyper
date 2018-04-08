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

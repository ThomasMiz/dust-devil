# Dust Devil

[![Crates.io](https://img.shields.io/crates/v/dust-devil.svg)](https://crates.io/crates/dust-devil)

Dust Devil is a SOCKS5 proxy server, the spiritual successor of [TornadoProxy](https://github.com/ThomasMiz/TornadoProxy.git), written in [Rust](https://www.rust-lang.org/) with the intent of learning said language. The project includes a custom TCP-based monitoring protocol called Sandstorm which allows managing and monitoring the server in real time. This protocol is implemented in the server, and a client application for it is provided too, including an advanced TUI (Terminal User Interface).

Dust Devil has the following features:

* Works on both Windows and Linux
* Support for IPv4 and IPv6 for both SOCKS5 and Sandstorm connections
* Support for SOCKS5 ([RFC 1928](https://www.rfc-editor.org/rfc/rfc1928)) TCP connections ("Connect" command only)
* Support for connecting to an IPv4/IPv6/Domainname. If a domain name's resolution yields multiple addresses, connection to these will be attempted in said order
* Support for the "NO AUTHENTICATION REQUIRED" and "USERNAME/PASSWORD" authentication methods, plus the ability to turn these on or off at any time
* User persistence to file (by default to `users.txt`) in a human-readable format (that's not intended to be secure)
* Can choose any arbitrary amount of sockets (address:port) to listen at for incoming SOCKS5 or Sandstorm connections
* Can choose the buffer size used for SOCKS5 connections (8KB by default)
* Detailed logging, by default to standard output, but may also output to a file, as well as metrics collection (total connections, bytes sent or received, etc)
* Extensive remote monitoring capabilities through the custom Sandstorm protocol, including:
  * Listing/registering/updating/deleting users
  * Shutting down the server
  * Listing/adding/removing a listening socket (for both SOCKS5 or Sandstorm)
  * Listing/enabling/disabling authentication methods
  * Getting/setting the buffer size
  * Real time logs/events and metrics

# The Sandstorm Protocol
This protocol, partly defined in [sandstorm_protocol.txt](https://github.com/ThomasMiz/dust-devil/blob/main/dust-devil-core/sandstorm_protocol.txt) (but with most of the struct serialization [defined in Rust](https://github.com/ThomasMiz/dust-devil/blob/main/dust-devil-core/src/serialize.rs)).

The `dust-devil-core` library crate contains the [Rust implementation of this protocol](https://github.com/ThomasMiz/dust-devil/tree/main/dust-devil-core/src/sandstorm) that is used by both the server and the client. This crate is also [available as standalone in crates.io](https://crates.io/crates/dust-devil-core) and [thoroughly documented in docs.rs](https://docs.rs/dust-devil-core/latest/dust_devil_core/).

Sandstorm is a TCP-based protocol and is a mix between pipelined request-response and asynchronous streams. The client sends requests to the server and gets responses back, however requests are not guaranteed to be handled nor answered by the server in the same order as they were sent. Multiple requests to add or remove users are guaranteed to be synchronized between themselves, but an open socket request in between those could be answered first, last, or anywhere in between. For more information on Sandstorm's request synchronization rules, check the "Pipelining" section of [sandstorm_protocol.txt](https://github.com/ThomasMiz/dust-devil/blob/main/dust-devil-core/sandstorm_protocol.txt).

In addition to this, the client may enable _event streaming_, which makes the server send an asynchronous stream of real-time events, which are interspersed with the request responses. These events are not simply strings with the log output (even though these events are internally used by the server to also generate the logs), but rather are serialized in a binary format with detailed information.

_Note: events are serialized in an efficient binary format, but still, if a client's connection to the server isn't fast enough to handle the rate at which events are generated, the client's connection will be abruptly terminated._

# Installation and Usage
The easiest way to install is with `cargo` from crates.io:
```
cargo install dust-devil
```

Or directly from GitHub:
```
cargo install --git https://github.com/ThomasMiz/dust-devil.git dust-devil
```

Either one of these will download and compile the server's code and all its dependencies. Once this is done, the server's executable will become available under the name `dust-devil`.

The monitoring client can be installed the same way:
```
cargo install dust-devil-sandstorm
```

Or directly from GitHub:
```
cargo install --git https://github.com/ThomasMiz/dust-devil.git sandstorm
```

Either one of these will download and compile the client's code and all its dependencies. Once this is done, the client's executable will become available under the name `sandstorm`.

## Server Usage
The server can be ran as-is without any setup, using default configuration, and will start outputting logs to standard output immediately:
```
$ dust-devil
[2024-02-23 16:08:39] Loading users from file users.txt
[2024-02-23 16:08:39] Error while loading users from file users.txt: IO error: The system cannot find the file specified. (os error 2)
[2024-02-23 16:08:39] Starting up with single default user admin:admin
[2024-02-23 16:08:39] Listening for socks5 client connections at [::]:1080
[2024-02-23 16:08:39] Listening for socks5 client connections at 0.0.0.0:1080
[2024-02-23 16:08:39] Listening for Sandstorm connections at [::]:2222
[2024-02-23 16:08:39] Listening for Sandstorm connections at 0.0.0.0:2222
```

The usage of the server is thoroughly explained in the help menu (`dust-devil --help`):
```
Usage: dust-devil [options...]
Options:
  -h, --help                      Display this help menu and exit
  -V, --version                   Display the version number and exit
  -v, --verbose                   Display additional information while running
  -s, --silent                    Do not print logs to stdout
  -d, --disable-events            Disables events, logs, and all data collection
  -o, --log-file <path>           Append logs to the specified file
  -l, --listen <address>          Specify a socket address to listen for incoming SOCKS5 clients
  -m, --management <address>      Specify a socket address to listen for incoming Sandstorm clients
  -U, --users-file <path>         Load and save users to/from this file
  -u, --user <user>               Adds a new user
  -A, --auth-enable <auth_type>   Enables an authentication method
  -a, --auth-disable <auth_type>  Disables an authentication method
  -b, --buffer-size <size>        Sets the size of the buffer for client connections

By default, the server will print logs to stdout, but not to any file. Logging may be enabled to
both stdout and to file at the same time. If a log sink is not fast enough to keep up the pace with
the server, then messages on said sink may be lost, indicated by an error message printed only to
said sink.

Socket addresses may be specified as an IPv4 or IPv6 address, or a domainname, and may include a
port number. The -l/--listen and -m/--management parameter may be specified multiple times to
listen on many addresses. If no port is specified, then the default port of 1080 will be used for
socks5 and 2222 for Sandstorm. If no --listen parameter is specified, then [::]:1080 and
0.0.0.0:1080 will be used, and if no Sandstorm sockets are specified, then [::]:2222 and
0.0.0.0:2222 will be used.

Users are specified in the same format as each line on the users file, but for regular users you
may drop the role character. For example, -u "pedro:1234" would have the same effect as --user
"#pedro:1234", and admins may be added with, for example "@admin:secret".

For enabling or disabling authentication, the available authentication types are "noauth" and
"userpass". All authentication methods are enabled by default.

The default buffer size is 8KBs. Buffer sizes may be specified in bytes ('-b 8192'), kilobytes
('-b 8K'), megabytes ('-b 1M') or gigabytes ('-b 1G' if you respect your computer, please don't)
but may not be equal to nor larger than 4GBs.


Examples:

Starts the server listening for SOCKS5 clients on all IPv4 addresses on port 1080 and Sandstorm
clients on all IPv6 addresses on port 2222 (the ports are implicit), writing logs to a logs.txt
file and creating a new admin user called "pedro" with password "1234":
    dust-devil -o logs.txt -l 0.0.0.0 -m [::] -u @pedro:1234

Starts the server with the default sockets listening for incoming SOCKS5 clients, but only allows
incoming management connections from localhost:3443. The buffer size for clients is set to 4096
bytes, logging and metrics are disabled:
    dust-devil -m localhost:3443 -b 4k -d -s

Starts the server with the default listening sockets, diasbles "noauth" authentication (to force
clients to authenticate with a username and password), and creates three users: Admin user
'Nicolás' with password '#nicorules', regular user 'Greg:orio with password 'holus', and regular
user '#tade0' with password 'tadaa':
    dust-devil -a noauth -u @Nicolás:#nicorules -u Greg\:orio::holus: -u ##tade0:tadaa
```

# Client Usage
The monitoring client requires at least the credentials parameter to be specified (as monitoring
clients must authenticate as an admin user with the server). The credentials may be specified via
parameter or via a `SANDSTORM_USER` environment variable. Unless specified with `-x <address>`, the
client connects to the server at `localhost:2222`.

The usage of the client is thoroughly explained in the help menu (`sandstorm --help`):
```
Usage: sandstorm [options...]
Options:
  -h, --help                      Display this help menu and exit
  -V, --version                   Display the version number and exit
  -v, --verbose                   Display additional information while running
  -s, --silent                    Do not print to stdout
  -x, --host <address>            Specify the server to connect to
  -c, --credentials <creds>       Specify the user to log in as, in user:password format
  -S, --shutdown                  Requests the server to shut down
  -l, --list-socks5               Requests the server sends a list of socks5 sockets
  -k, --add-socks5 <address>      Requests the server opens a new socks5 socket
  -r, --remove-socks5 <address>   Requests the server removes an existing socks5 socket
  -L, --list-sandstr              Requests the server sends a list of Sandstorm sockets
  -K, --add-sandstr <address>     Requests the server opens a new Sandstorm socket
  -R, --remove-sandstr <address>  Requests the server removes an existing Sandstorm socket
  -t, --list-users                Requests the server adds a new user
  -u, --add-user <user>           Requests the server adds a new user
  -p, --update-user <updt_user>   Requests the server updates an existing user
  -d, --delete-user <username>    Requests the server deletes an existing user
  -z, --list-auth                 Requests the server sends a list of auth methods
  -A, --auth-enable <auth_type>   Requests the server enables an authentication method
  -a, --auth-disable <auth_type>  Requests the server disables an authentication method
  -m, --get-metrics               Requests the server sends the current metrics
  -B, --get-buffer-size           Requests the server sends the current buffer size
  -b, --set-buffer-size <size>    Requests the server changes its buffer size
  -w, --meow                      Requests a meow ping to the server
  -o, --output-logs               Remain open and print the server's logs to stdout
  -i, --interactive               Remains open with an advanced terminal UI interface

Socket addresses may be specified as an IPv4 or IPv6 address, or a domainname, and may include a
port number. If no port is specified, then the appropriate default will be used (1080 for Socks5
and 2222 for Sandstorm). If no -x/--host parameter is specified, then localhost:2222 will be used.

Credentials may be specified with the -c/--credentials argument, in username:password format. If no
credentials argument is specified, then the credentials will be taken from the SANDSTORM_USER
environment variable, which must follow the same format.

When adding a user, it is specified in the (role)?user:password format. For example, "#carlos:1234"
represents a regular user with username "carlos" and password "1234", and "@josé:4:4:4" represents
an admin user with username "josé" and password "4:4:4". If the role char is omitted, then a
regular user is assumed. Updating an existing user work much the same way, but the role char or
password may be omitted. Only the fields present will be updated, those omitted will not be
modified. To specify an username that contains a ':' character, you may escape it like so:
"#chi\:chí:4:3:2:1" (this produces a regular user "chi:chí" with password "4:3:2:1"). When deleting
an user, no escaping is necessary, as only the username is specified.
For enabling or disabling authentication, the available authentication types are "noauth" and
"userpass".

Buffer sizes may be specified in bytes ('-b 8192'), kilobytes ('-b 8K'), megabytes ('-b 1M') or
gigabytes ('-b 1G' if you respect your computer, please don't) but may not be equal to nor larger
than 4GBs.

The requests are done in the order in which they're specified and their results printed to stdout
(unless -s/--silent is specified). Pipelining will be used, so the requests are not guaranteed to
come back in the same order. The only ordering guarantees are those defined in the Sandstorm
protocol (so, for example, list/add/remove socks5 sockets operations are guaranteed to be handled
in order and answered in order, but an add user request in the middle of all that may not come back
in the same order).

The -o/--output-logs and -i/--interactive modes are mutually exclusive, only one may be enabled.


Examples:

Connects to the server at 192.168.1.1:2222 (the port is implicit), logs in with user 'admin'
password 'admin', and requests three consecutive meow pings:
    sandstorm -x 192.168.1.1 -c admin:admin -w -w -w

Connects to the server at 10.4.20.1:8900, logs in with user 'pedro' password '1234', then requests
adding the admin user 'pedro' with password '1234', updates the role of user 'marcos' to regular
(the password is not changed), deletes the user 'josé', and finally lists all the users:
    sandstorm -x 10.4.2.1:8900 -c pedro:1234 -u @pedro:1234 -p #marcos -d josé -t

Connects to the server at localhost:2222 (implicit) with user 'admin' and requests the current
buffer size and metrics:
    sandstorm -c admin:admin -B -m

Connects to the server at localhost:2222 (implicit) with user 'admin' and shuts down the server:
    sandstorm -c admin:admin -S

Connects to the server at localhost:2222 (implicit) with user 'admin', requests the list of
listening SOCKS5 sockets, and then leaves the connection open streaming server events and printing
them to stdout until manually closed (with Ctrl-C):
    sandstorm -c admin:admin -l -o

Connects to the server at localhost:2222 (implicit) with user 'admin', sends three meow pings and
then opens the interactive TUI (Terminal User Interface)
    sandstorm -c admin:admin -w -w -w -i
```

# Monitoring TUI
The aformentioned monitoring client provides an "interactive" mode, in which the application takes
manual control of the terminal and opens up an advanced interface:

![monitoring_client](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/sandstorm.png)

This monitoring client provides a more friendly, but still fully featured interface for monitoring
the server, displaying real-time metrics, events, and a usage history graph.

# Gallery

![firefox_example](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/firefox.png)

![making_my_isp_hate_me](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/making_my_isp_hate_me.png)

![graph_expanded](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/graph_expanded.png)

![socks5_popup](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/socks5_popup.png)

![users_popup](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/users_popup.png)

![auth_methods_popup](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/auth_methods_popup.png)

![buffer_size_popup](https://raw.githubusercontent.com/ThomasMiz/dust-devil/main/images/buffer_size_popup.png)

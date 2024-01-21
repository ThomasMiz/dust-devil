//! Shared sources used in the dust-devil socks5 server and monitoring clients for implementing the
//! Sandstorm monitoring protocol.
//!
//! # The Sandstorm monitoring protocol
//! The dust-devil server uses a custom monitoring protocol called Sandstorm. Sandstorm is a TCP
//! protocol that is partly a request-response protocol, and partly a server-side stream of events.
//! A better description of the protocol is provided in the
//! [sandstorm_protocol.txt](https://github.com/ThomasMiz/dust-devil/blob/main/dust-devil-core/sandstorm_protocol.txt)
//! file.
//!
//! The client can request actions to the server, such as adding or removing users, getting
//! metrics, shutting down, etc. The server will then perform these actions and send responses back
//! to the client indicating their results.
//!
//! This "stream of events" can be enabled by the client, and while enabled the server will send
//! asynchronous events to the client. These events can be used, for example, as logging (in fact,
//! if logging is enabled in the dust-devil server, logs are just a text representation of the
//! events).
//!
//! # This crate
//! This crate contains common files used for serializing and deserializing requests and responses
//! for the Sandstorm protocol from raw bytes. All serializers and deserializers are prepared to
//! receive any type implementing [`AsyncRead`][tokio::io::AsyncRead] or
//! [`AsyncWrite`][tokio::io::AsyncWrite] from [`tokio::io`] and are async.
//!
//! All the serializing revolves around the [`ByteRead`][serialize::ByteRead] and
//! [`ByteWrite`][serialize::ByteWrite] traits. These are defined in the [`serialize`] module and
//! define async `read` and `write` functions.

pub mod logging;
pub mod sandstorm;
pub mod serialize;
pub mod socks5;
pub mod u8_repr_enum;
pub mod users;

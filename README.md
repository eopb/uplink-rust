# uplink-rust

Storj Uplink Rust bindings for the Rust programming language.

## Repository layout

Following the conventions used for creating Rust bindings through [bindgen][bindgen], this repository contains two crates:

* The [`uplink-sys`](uplink-sys) which is the unsafe Rust bindings auto-generated by [bindgen][bindgen].
* The [`uplink`](uplink) which is the safe and idiomatic Rust binding build on top of the `uplink-sys`.

[bindgen]: https://github.com/rust-lang/rust-bindgen/

Each crate matches a root's child directory with the same name and each directory has its own README which provides more detailed information and its current status.

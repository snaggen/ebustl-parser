EBU-STL subtitling format in Rust
=================================
[![crates.io](https://img.shields.io/crates/v/ebustl-parser.svg)](https://crates.io/crates/ebustl-parser)
[![docs.rs](https://docs.rs/ebustl-parser/badge.svg)](https://docs.rs/ebustl-parser/latest/ebustl-parser/)

A basic implementation of a parser for the [EBU-STL subtitling file format](https://tech.ebu.ch/docs/tech/tech3264.pdf).

This is a fork of [ebustl] https://github.com/tytouf/ebustl-rs, for maintenance purposes

Example:
```rust
use ebustl_parser::parse_stl_from_file;
use std::env;
use std::process;

fn main() {
    let stl = parse_stl_from_file("/path/to/subtiltle.stl").expect("Parse stl from file");
    println!("{:?}", stl);
}

```

License: [EUPL](LICENSE.EUPL)

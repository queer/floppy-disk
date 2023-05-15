# floppy-disk

*floppy disk* is a WIP, async-only filesystem facade for Rust.

## What?

Have you ever worked with `std::fs`? `tokio::fs`? Then you've probably realised
that testing filesystem code is difficult and sometimes scary. Is that
`fs::remove_dir_all` *really* safe to run?

The point of *floppy disk* is to fix this. Rather than always using the real
filesystem, *floppy disk* lets you choose a backend for your filesystem access,
via the `FloppyDisk` trait. Current implementations include in-memory and real
filesystem via Tokio. This way, you can use the real filesystem when you need,
but have your tests hit a fake in-memory filesystem instead.

## Features

- Pluggable filesystem backends
  - In-memory (WIP)
  - Tokio
- Write-your-own with the `FloppyDisk` trait
- Fully-async
  - Light evil involved

### Caveats

- ***floppy disk* is a 0.x.y project!** You probably don't want to use it in
  production.
- async-only! There is some small bridging to sync code, like `MemFile`
  implementing `Read`/`Write`/`Seek`, but this is mostly a hack to make
  working with sync-only external libraries (ex. `ar`) easier.
- in-memory fs may not be performant-enough

## Example usage

*floppy disk* attempts to recreate the `std::fs` API 1:1, with the caveat of
being async-only.

```rust
let fs = ...; // MemFloppyDisk::new() | TokioFloppyDisk::new()
fs.create_dir_all("/foo/bar").await?;
fs.write("/foo/bar/baz.txt", b"hello world").await?;
let contents = fs.read_to_string("/foo/bar/baz.txt").await?;
assert_eq!(contents, "hello world");
```
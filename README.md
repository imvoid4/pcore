# pcore
> Note: This project is under active development and is not yet complete.

pcore is a fast CPU-based core for a raster paint app. It provides the necessary functions for handling layers, undo state, brushes and shapes, ready to be used with any UI toolkit.

## Current benchmark highlights

The full benchmark results can be found at target/criterion/report/index.html.

| Operation | Speed |
|---|---:|
| Filling the whole canvas | 39.547 ms |
| 1000px brush stroke with a radius of 8 | 973.86 us |
| Single pixel write (no blend) | 1.56 ns |
| Single pixel write with blend | 4.06 ns |
| Blended 4096x4096 ellipse at 128 alpha | 669.76 us |

## Building

Requirements:

- ```git```
- Rust toolchain
- ```cargo```

First, clone the repo:

```
git clone https://github.com/imvoid4/pcore.git
cd pcore
```

Build

```
cargo build --release
```

(Optional) Benchmark

```
cargo bench
```

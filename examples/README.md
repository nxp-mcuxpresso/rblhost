# McuBoot examples
This folder contains examples for bindings or direct usage of McuBoot library. For instructions on how to run, refer to the sections below.

Every example has a comment at the start to describe what it does and may contain some notes about the binding or example.

## Rust
Rust examples can be run with the following Cargo command:
```bash
cargo run --example <filename> -- <arguments>
```
To run `get_property.rs` with `COM3` device, you would write the following:
```bash
cargo run --example get_property -- COM3
```

## Python
Follow [instructions for installing the Python package](../README.md#building-python-bindings). Once you do that, run all examples with `python` command. For `get_property.py` with `COM3` device from this folder, use the following:
```bash
python python/get_property.py COM3
```

## C
Follow [instructions on compiling C libraries](../README.md#building-c-bindings). Then you can build examples using [CMake](https://cmake.org/). To do so, install cmake and any modern C and C++ compiler. Then, inside the [c folder](c/), run:
```bash
mkdir build
cd build
cmake ..
cmake --build .
```
This will build examples either in the `build` directory directly or `Debug` directory, depending on your compiler.


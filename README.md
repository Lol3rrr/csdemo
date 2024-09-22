# CSDemo
A parser for cs2 demo files

## Requirements
- `protoc` needs to be installed locally
- `Protobufs` submodule needs to be checked out using git as well

## Benchmarking
Currently there are no proper benchmarks.
Right now one can compile the given examples and run them using something like
[hyperfine](https://github.com/sharkdp/hyperfine)

## Profiling
Similar to the Benchmarking section, the best approach currently is to compile
the examples with debug information and then run them under a profiling tool of
your choice, like [samply](https://github.com/mstange/samply)

# Simple sound shader player by Rust

[![Crates.io](https://img.shields.io/crates/v/sound-shader.svg)](https://crates.io/crates/sound-shader) [![Docs.rs](https://docs.rs/sound-shader/badge.svg)](https://docs.rs/sound-shader)

A simple CLI application for playing sound GLSL shader by Rust.

## How to start

Clone repository, and run

```bash
cd sound-shader
mkdir workspace
cd workspace
cargo run -- --init
cargo run
```

## Examples

There are some examples in `examples`. A very cool sound shader by Inigo Quilez can be played with the following command!

```bash
cargo run -- examples/pirates.comp
```

## Help

```bash
sound-shader 0.1.0
Yoshinori Tanimura <yotabaito@gmail.com>
Simple sound shader player

USAGE:
    sound-shader.exe [FLAGS] [OPTIONS] [--] [FILE]

FLAGS:
    -h, --help       Prints help information
        --init       init default config file "default.json" and prepare sample shader source "sample.comp"
    -V, --version    Prints version information

OPTIONS:
    -c, --config <FILE>          read configuration json
    -o, --output <FILE>          recording wav file
    -r, --resources <FILE>...    add audio resource, wav is supported.
    -s, --silent <SECONDS>       not play, just recording.

ARGS:
    <FILE>    run shader source
```

The json settings specified in `--cofig` will be treated as the basic settings.
If other arguments are specified for shaders, resources, or output, the settings will be overridden at runtime.
For example, if only a different shader is specified in the other arguments, the result will be output to the destination specified json.
If no arguments are specified, `default.json` will be loaded, but if other arguments are specified, they will be ignored;
if you want to use `default.json` as the default setting, you must load it explicitly with `--config`.

## License

This crate is distributed under Apach-2.0. However, this repository contains some resources that are distributed on a non-commercial basis only.
Specifically, the following files are distributed on a non-commercial basis.

- `resources/*.wav`
- `examples/pirates.comp`

## Operating environment

The unit tests and examples are done on my Windows and Mac machines.
Since this program using both GPU and audio devices, I haven't established a valid CI.

## Future works

- loop back
- other audio resources: flac, mp3, and so on.

# aeris

**Unbounded package management.** A graphical package manager built with Rust and [GPUI](https://gpui.rs).

## Overview

Aeris is a desktop GUI for searching, installing, updating, and removing
packages. It talks to package backends through an adapter layer, so a
single interface can eventually front many package managers.

Today the shipping backend is [soar](https://github.com/pkgforge/soar).
A WebAssembly adapter system is included so additional backends can be
added as plugins without rebuilding Aeris. Support for more backends is
planned, not yet complete.

## Features

- Browse and search packages from configured repositories
- Install, update, and remove packages
- View installed packages and available updates
- Declarative manifest view: edit `packages.toml`, preview the diff, and apply
- Per package detail panel with source, build, and option fields
- Live progress for running operations

## Install

### Portable binary (recommended)

Each release ships a single self-contained executable built with
[onelf](https://github.com/QaidVoid/onelf). It bundles its own libraries
and runs on most Linux systems without installing anything.

Download `aeris-x86_64-linux.onelf` from the
[latest release](https://github.com/QaidVoid/aeris/releases/latest),
then:

```sh
chmod +x aeris-x86_64-linux.onelf
./aeris-x86_64-linux.onelf
```

Nightly builds are published on the rolling
[`nightly`](https://github.com/QaidVoid/aeris/releases/tag/nightly) tag.

### From source

Requires a Rust toolchain and the usual GPUI build dependencies
(fontconfig, freetype, libxcb, libxkbcommon, wayland, and alsa headers).

```sh
cargo build --release
./target/release/aeris
```

A Nix flake is provided:

```sh
nix develop
```

## Contributing

Contributions are welcome. Please feel free to open issues or pull requests.

## License

MIT

# aeris

**Unbounded package management.** A graphical package manager built with Rust and [GPUI](https://gpui.rs).

## Overview

Aeris is a desktop GUI for searching, installing, updating, and removing
packages. It drives each package manager as a command, described by a TOML
manifest, so one interface fronts several of them at once.

[soar](https://github.com/pkgforge/soar) is built in and describes itself, so
Aeris always drives the soar that is actually installed. Other managers are
added at runtime from the
[adapter registry](https://github.com/pkgforge/aeris-registry), so supporting
one is a matter of writing a manifest rather than changing Aeris.

## Features

- Search every enabled manager at once, ranked by how well each result answers
- Install, update, and remove packages
- Work per user or system wide, for a manager that offers both
- Add adapters from the registry, refreshed on an interval and offered as updates
- View installed packages and available updates
- Declarative manifest view: edit `packages.toml`, preview the diff, and apply
- Per package detail panel with source, build, and option fields
- Live progress, including answering a manager that stops to ask something

## Install

### Portable binary (recommended)

Each release ships a single self-contained executable built with
[onelf](https://github.com/QaidVoid/onelf). It bundles its own libraries
and runs on most Linux systems without installing anything.

Download `aeris-x86_64-linux.onelf` from the
[latest release](https://github.com/pkgforge/aeris/releases/latest),
then:

```sh
chmod +x aeris-x86_64-linux.onelf
./aeris-x86_64-linux.onelf
```

Nightly builds are published on the rolling
[`nightly`](https://github.com/pkgforge/aeris/releases/tag/nightly) tag.

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

## Adapters

An adapter is a TOML manifest naming the arguments for each operation and how
to read what comes back, so a manager that already answers in JSON needs
nothing more than a description. A manifest also says how the manager acts
system wide, which settings it accepts, and whether an operation needs a
terminal.

Manifests are read from, in order:

```
~/.local/share/aeris/adapters
/usr/local/share/aeris/adapters
./adapters
```

The Adapters page installs them from the registry and checks for newer ones.
See [pkgforge/aeris-registry](https://github.com/pkgforge/aeris-registry) for
the published manifests and the schema.

## Contributing

Contributions are welcome. Please feel free to open issues or pull requests.

## License

MIT

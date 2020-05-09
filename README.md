# [WIP] pod-compose

> docker-compose for podman!

## Installation

With rust and cargo installed:

```sh
$ cargo install --git https://github.com/martinrlilja/pod-compose.git pod-compose
```

If you don't have rust installed, [rustup.rs](https://rustup.rs/) is usually
the easiest/best way to install it. Your OS's package manager might also have
it.

## Features

This is a very barebones implementation of docker-compose for podman. Very few
features are supported at this point, if you want to know if a feature is
supported, the answer is probably no. That said - contributions are welcome!

### Commands

 * `up -d`, does not support non-detached mode, but it will recreate your
    containers if something changes.
 * `stop`
 * `down`
 * `build`
 * `--remove-orphans`

### docker-compose.yml

 * Looks for your docker-compose.yml file recursively up the file hierarchy.
 * `build`, `image`
 * `replicas`

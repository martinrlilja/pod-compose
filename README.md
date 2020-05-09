# [WIP] pod-compose

> docker-compose for podman!

## Installation

With rust and cargo installed:

```
$ cargo install --git https://github.com/martinrlilja/pod-compose.git pod-compose
```

If you don't have it, [rustup.rs](https://rustup.rs/) is usually the
easiest/best way to install it. Your OS's package manager might also have it.

## Features

This is a very barebones implementation of docker-compose for podman. At this
point it's probably easier to specify what it supports rather than what it does
not.

 * `up -d`, currently does not build images, or support non-detached mode.
 * `stop`
 * `down`
 * `--remove-orphans`

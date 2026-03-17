# Pion

Simple rust userspace root service for Binder. Run it once, use it like unix domain sockets through files!

The goal is just to provide unix domain socket semantics (e.g. connect/bind) for binder objects as the context manager... that way we can integrate the existing $XDG_RUNTIME_DIR and such... and you can bind binder objects to existing socket files too ;3


First, build the project

```
cargo build
```

Then, run the service itself

```
sudo ./target/debug/pion-binder
```

And while that's running, you can run the example

```
cargo run --example simple_service
```

Happy binding! :3

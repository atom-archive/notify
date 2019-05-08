# Notify Subprocess

This is a simple Rust executable that wraps the `notify` crate.

On the main thread, it creates a `Supervisor`, which owns a `Watcher` implementation as well as an array of `Watch` objects. When the `Supervisor` is constructed, we spawn a background thread that reads events sent by the `notify` crate on a channel. The thread shares a synchronized reference to the watchers array.

When the background thread receives events, we iterate through the watches and allow them to process events. When a watch finds events that fall under its `root`, we produce an `Event` which is pushed to a batch of `Outgoing::WatchEvents` and emitted as JSON.

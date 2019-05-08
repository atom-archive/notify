# @atom/notify

This Node.js module recursively-observes directory trees on the file system on macOS, Windows, and Linux. It delegates the heavy lifting to the Rust [`notify`](https://github.com/passcod/notify) crate, which is wrapped in a simple executable that is spawned as a subprocess.

## API

```js
const Watcher = require("@atom/notify");

// Construct the watcher
const watcher = new Watcher();

// Watch a directory path
const watch = await watcher.watch("/my/huge/directory", (events) => {
  /* handle array of events */
});

// Remove the watch
await watch.dispose();

// Shut down the watcher completely
await watcher.kill();
```

The callback passed to watch is called with an array of event objects. Each event object takes one of the following forms:

* `{action: 'modified', path: string}`
* `{action: 'created', path: string}`
* `{action: 'deleted', path: string}`
* `{action: 'renamed', oldPath: string, path: string}`
* `{action: 'error', path: string, description: string}`

### Error handling

The `watcher` crate can emit errors. If these errors are associated with paths, they are forwarded as events to your watch callback. If an error is encountered that's not associated with a path, it is passed to an `onError` callback that is passed as a parameter to the `Watcher` constructor.

```js
const watcher = new Watcher({
  onError: (error) => { /* global error handling */ },
});
```

### Polling mode

By default, the an appropriate implementation of the watcher is selected for the current platform. You can also generate events by *polling* the file system. To do this, pass a `pollInterval` to the `Watcher` constructor with your desired duration between polls in milliseconds:

```js
const watcher = new Watcher({pollInterval: 1000});
```

## Project structure

This library spawns a subprocess that speaks a simple line-oriented JSON protocol over stdin and stdout. The Rust source for the subprocess is located in [`subprocess`](./subprocess).

When this module is installed, a platform-appropriate binary is automatically downloaded from a GitHub release that matches the `version` field in the `package.json`. This avoids the need for installers to have a Rust toolchain installed on their system.

To produce the release artifacts, this project is associated with an [Azure build pipeline](https://github.visualstudio.com/Atom/_build?definitionId=50) that automatically builds the subprocess binary for all three platforms whenever a release tag of the format `vX.Y.Z` is pushed to the repository. Each binary is suffixed with the platform name and uploaded to the GitHub release.

To publish a new version, once you have a green build, run `npm version` to produce a new version tag and push it to the repository. Once Azure builds and uploads the subprocess artifacts to a new release, you can run `npm publish`. If you run `npm publish` prior to the artifact upload, users won't be able to download the appropriate binaries on installation for a brief window.

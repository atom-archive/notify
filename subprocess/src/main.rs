use dunce;
use notify::{DebouncedEvent, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use structopt::StructOpt;

type RequestId = usize;
type WatchId = usize;

#[derive(StructOpt, Debug)]
#[structopt(name = "subprocess")]
struct Opt {
    /// Enable polling mode with the specified interval in milliseconds
    #[structopt(long = "poll-interval")]
    poll_interval: Option<u64>,
}

struct Supervisor<W> {
    watcher: W,
    watches: Arc<Mutex<Vec<Watch>>>,
}

struct Watch {
    ids: Vec<usize>,
    root: PathBuf,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum Incoming {
    #[serde(rename_all = "camelCase")]
    Watch {
        request_id: RequestId,
        watch_id: WatchId,
        root: PathBuf,
    },
    #[serde(rename_all = "camelCase")]
    Unwatch {
        request_id: RequestId,
        watch_id: WatchId,
    },
    #[serde(rename_all = "camelCase")]
    UnwatchAll { request_id: RequestId },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum Outgoing {
    #[serde(rename_all = "camelCase")]
    OkResponse {
        request_id: RequestId,
    },
    #[serde(rename_all = "camelCase")]
    ErrorResponse {
        request_id: WatchId,
        description: String,
    },
    #[serde(rename_all = "camelCase")]
    WatchEvents {
        watch_id: WatchId,
        events: Vec<Event>,
    },
    WatcherError {
        description: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "action")]
#[serde(rename_all = "camelCase")]
enum Event {
    Modified {
        path: PathBuf,
    },
    Created {
        path: PathBuf,
    },
    Deleted {
        path: PathBuf,
    },
    #[serde(rename_all = "camelCase")]
    Renamed {
        path: PathBuf,
        old_path: PathBuf,
    },
    Error {
        path: PathBuf,
        description: String,
    },
}

impl<W: Watcher> Supervisor<W> {
    fn new(delay: Duration) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let watcher = W::new(tx, delay)?;
        let watches = Arc::new(Mutex::new(Vec::new()));

        let watches_2 = watches.clone();
        thread::spawn(move || {
            loop {
                if let Ok(event) = rx.recv() {
                    let mut events = vec![event];
                    events.extend(rx.try_iter()); // Collect more pending events without blocking
                    Self::notify(&watches_2, events);
                } else {
                    break;
                }
            }
        });

        Ok(Self { watcher, watches })
    }

    fn notify(watches: &Arc<Mutex<Vec<Watch>>>, events: Vec<DebouncedEvent>) {
        for watch in watches.lock().unwrap().iter() {
            watch.notify(&events)
        }

        // Emit errors that aren't associated with paths as top-level events
        for event in &events {
            if let DebouncedEvent::Error(error, None) = event {
                emit_json(Outgoing::WatcherError {
                    description: String::from(error.description()),
                })
            }
        }
    }

    fn handle_requests(&mut self) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let request = serde_json::from_str(&line.unwrap()).unwrap();
            self.handle_request(request);
        }
    }

    fn handle_request(&mut self, request: Incoming) {
        match request {
            Incoming::Watch {
                request_id,
                watch_id,
                root,
            } => self.watch(request_id, watch_id, root),
            Incoming::Unwatch {
                request_id,
                watch_id,
            } => self.unwatch(request_id, watch_id),
            Incoming::UnwatchAll { request_id } => self.unwatch_all(request_id),
        }
    }

    fn watch(&mut self, request_id: RequestId, watch_id: WatchId, root: PathBuf) {
        let mut watches = self.watches.lock().unwrap();

        match fs::canonicalize(&root) {
            Ok(root) => {
                if let Some(watch) = watches.iter_mut().find(|watch| watch.root == root) {
                    watch.ids.push(watch_id);
                    emit_json(Outgoing::OkResponse { request_id });
                } else {
                    if let Err(error) = self.watcher.watch(&root, RecursiveMode::Recursive) {
                        emit_json(Outgoing::ErrorResponse {
                            request_id,
                            description: error.description().to_string(),
                        });
                    } else {
                        watches.push(Watch {
                            root,
                            ids: vec![watch_id],
                        });
                        emit_json(Outgoing::OkResponse { request_id });
                    }
                }
            }
            Err(error) => emit_json(Outgoing::ErrorResponse {
                request_id,
                description: error.description().to_string(),
            }),
        }
    }

    fn unwatch(&mut self, request_id: RequestId, watch_id: WatchId) {
        let mut watches = self.watches.lock().unwrap();

        let mut found_id = false;
        let mut unwatch_error = None;
        let mut index_to_remove = None;

        for (i, watch) in watches.iter_mut().enumerate() {
            if let Some(j) = watch.ids.iter().position(|id| *id == watch_id) {
                found_id = true;
                watch.ids.remove(j);
                if watch.ids.is_empty() {
                    index_to_remove = Some(i);
                    if let Err(error) = self.watcher.unwatch(&watch.root) {
                        unwatch_error = Some(error);
                    }
                }
            }
        }

        if let Some(i) = index_to_remove {
            let removed = watches.remove(i);

            // On Linux, unwatching a directory seems to destroy all watches on descendant
            // directories, so we rewatch any descendant directories that are being monitored.
            if cfg!(target_os = "linux") {
                for watch in watches.iter() {
                    if watch.root.starts_with(&removed.root) {
                        if let Err(error) =
                            self.watcher.watch(&watch.root, RecursiveMode::Recursive)
                        {
                            emit_json(Outgoing::ErrorResponse {
                                request_id,
                                description: format!(
                                    "Error re-watching descendant of unwatched directory: {:?}",
                                    error
                                ),
                            });
                            return;
                        }
                    }
                }
            }
        }

        if let Some(error) = unwatch_error {
            emit_json(Outgoing::ErrorResponse {
                request_id,
                description: format!("Error unwatching: {:?}", error),
            });
        } else if found_id {
            emit_json(Outgoing::OkResponse { request_id });
        } else {
            emit_json(Outgoing::ErrorResponse {
                request_id,
                description: format!("No watch found for id: {:?}", watch_id),
            });
        }
    }

    fn unwatch_all(&mut self, request_id: RequestId) {
        let mut watches = self.watches.lock().unwrap();

        for watch in watches.drain(..) {
            if let Err(error) = self.watcher.unwatch(&watch.root) {
                emit_json(Outgoing::ErrorResponse {
                    request_id,
                    description: format!("Error unwatching {:?}: {:?}", watch.root, error),
                });
                return;
            }
        }

        emit_json(Outgoing::OkResponse { request_id });
    }
}

impl Watch {
    fn notify(&self, events: &[DebouncedEvent]) {
        let mut batch = Vec::new();

        for event in events {
            match event {
                DebouncedEvent::Create(path) => {
                    if path.starts_with(&self.root) {
                        batch.push(Event::created(path));
                    }
                }
                DebouncedEvent::Write(path) => {
                    if path.starts_with(&self.root) {
                        batch.push(Event::modified(path));
                    }
                }
                DebouncedEvent::Remove(path) => {
                    if path.starts_with(&self.root) {
                        batch.push(Event::deleted(path));
                    }
                }
                DebouncedEvent::Rename(old_path, new_path) => {
                    match (
                        old_path.starts_with(&self.root),
                        new_path.starts_with(&self.root),
                    ) {
                        (true, true) => batch.push(Event::renamed(old_path, new_path)),
                        (true, false) => batch.push(Event::deleted(old_path)),
                        (false, true) => batch.push(Event::created(new_path)),
                        (false, false) => {}
                    }
                }
                DebouncedEvent::Error(error, path) => {
                    if let Some(path) = path {
                        if path.starts_with(&self.root) {
                            batch.push(Event::error(path, error));
                        }
                    }
                }
                DebouncedEvent::NoticeWrite(_path) => {}
                DebouncedEvent::NoticeRemove(_path) => {}
                DebouncedEvent::Chmod(_path) => {}
                DebouncedEvent::Rescan => {}
            }
        }

        if !batch.is_empty() {
            for id in &self.ids {
                emit_json(Outgoing::WatchEvents {
                    watch_id: *id,
                    events: batch.clone(),
                });
            }
        }
    }
}

impl Event {
    fn modified(path: &Path) -> Self {
        Event::Modified {
            path: dunce::simplified(path).into(),
        }
    }
    fn created(path: &Path) -> Self {
        Event::Created {
            path: dunce::simplified(path).into(),
        }
    }
    fn deleted(path: &Path) -> Self {
        Event::Deleted {
            path: dunce::simplified(path).into(),
        }
    }
    fn renamed(old_path: &Path, new_path: &Path) -> Self {
        Event::Renamed {
            path: dunce::simplified(new_path).into(),
            old_path: dunce::simplified(old_path).into(),
        }
    }
    fn error(path: &Path, error: &notify::Error) -> Self {
        Event::Error {
            path: dunce::simplified(path).into(),
            description: String::from(error.description()),
        }
    }
}

fn emit_json(message: Outgoing) {
    println!("{}", &serde_json::to_string(&message).unwrap());
}

fn main() {
    let opt = Opt::from_args();

    if let Some(poll_interval) = opt.poll_interval {
        match Supervisor::<PollWatcher>::new(Duration::from_millis(poll_interval)) {
            Ok(mut supervisor) => supervisor.handle_requests(),
            Err(error) => {
                emit_json(Outgoing::WatcherError {
                    description: String::from(error.description()),
                });
                eprintln!("Error creating notify watcher: {:?}", error);
            }
        }
    } else {
        match Supervisor::<RecommendedWatcher>::new(Duration::from_millis(100)) {
            Ok(mut supervisor) => supervisor.handle_requests(),
            Err(error) => {
                emit_json(Outgoing::WatcherError {
                    description: String::from(error.description()),
                });
                eprintln!("Error creating notify watcher: {:?}", error);
            }
        }
    };
}

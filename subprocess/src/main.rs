use dunce;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

type WatchId = usize;

struct Supervisor {
    watcher: RecommendedWatcher,
    watches: Arc<Mutex<Vec<Watch>>>,
}

struct Watch {
    ids: Vec<WatchId>,
    root: PathBuf,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum Request {
    Watch { id: WatchId, root: PathBuf },
    Unwatch { id: WatchId },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum Response {
    Ok { id: WatchId },
    Error { id: WatchId, description: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventBatch {
    watch_id: WatchId,
    events: Vec<Event>,
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
}

impl Supervisor {
    fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let watcher = notify::watcher(tx, Duration::from_millis(300))?;
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
    }

    fn handle_requests(&mut self) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let request = serde_json::from_str(&line.unwrap()).unwrap();
            self.handle_request(request);
        }
    }

    fn handle_request(&mut self, request: Request) {
        match request {
            Request::Watch { id, root } => self.watch(id, root),
            Request::Unwatch { id } => self.unwatch(id),
        }
    }

    fn watch(&mut self, id: WatchId, root: PathBuf) {
        let mut watches = self.watches.lock().unwrap();

        match fs::canonicalize(&root) {
            Ok(root) => {
                if let Some(watch) = watches.iter_mut().find(|watch| watch.root == root) {
                    watch.ids.push(id);
                    emit_json(Response::Ok { id });
                } else {
                    if let Err(error) = self.watcher.watch(&root, RecursiveMode::Recursive) {
                        emit_json(Response::Error {
                            id,
                            description: error.description().to_string(),
                        });
                    } else {
                        watches.push(Watch {
                            root,
                            ids: vec![id],
                        });
                        emit_json(Response::Ok { id });
                    }
                }
            }
            Err(error) => emit_json(Response::Error {
                id,
                description: error.description().to_string(),
            }),
        }
    }

    fn unwatch(&mut self, id: WatchId) {
        let mut watches = self.watches.lock().unwrap();

        let mut found_id = false;
        let mut unwatch_error = None;
        let mut index_to_remove = None;

        for (i, watch) in watches.iter_mut().enumerate() {
            if let Some(j) = watch.ids.iter().position(|watch_id| *watch_id == id) {
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
            watches.remove(i);
        }

        if let Some(error) = unwatch_error {
            emit_json(Response::Error {
                id,
                description: format!("Error unwatching: {:?}", error),
            });
        } else if found_id {
            emit_json(Response::Ok { id });
        } else {
            emit_json(Response::Error {
                id,
                description: format!("No watch found for id: {:?}", id),
            });
        }
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
                DebouncedEvent::NoticeWrite(_path) => {}
                DebouncedEvent::NoticeRemove(_path) => {}
                DebouncedEvent::Chmod(_path) => {}
                DebouncedEvent::Rescan => {}
                DebouncedEvent::Error(_error, _path) => {} // TODO: Error handling
            }
        }

        if !batch.is_empty() {
            for id in &self.ids {
                emit_json(EventBatch {
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
}

fn emit_json<T: Serialize>(message: T) {
    println!("{}", &serde_json::to_string(&message).unwrap());
}

fn main() {
    let mut supervisor = Supervisor::new().unwrap();
    supervisor.handle_requests();
}

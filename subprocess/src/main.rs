use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

type WatchId = usize;

struct Supervisor {
    watcher: RecommendedWatcher,
    watches: Arc<Mutex<HashMap<WatchId, Watch>>>,
}

struct Watch {
    id: WatchId,
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
#[serde(tag = "action")]
#[serde(rename_all = "camelCase")]
enum Event {
    #[serde(rename_all = "camelCase")]
    Modified { watch_id: WatchId, path: PathBuf },
    #[serde(rename_all = "camelCase")]
    Created { watch_id: WatchId, path: PathBuf },
    #[serde(rename_all = "camelCase")]
    Deleted { watch_id: WatchId, path: PathBuf },
    #[serde(rename_all = "camelCase")]
    Renamed {
        watch_id: WatchId,
        path: PathBuf,
        old_path: PathBuf,
    },
}

impl Supervisor {
    fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let watcher = notify::watcher(tx, Duration::from_millis(300))?;
        let watches = Arc::new(Mutex::new(HashMap::new()));

        let watches_2 = watches.clone();
        thread::spawn(move || {
            for event in rx {
                Self::notify(&watches_2, event);
            }
        });

        Ok(Self { watcher, watches })
    }

    fn notify(watches: &Arc<Mutex<HashMap<WatchId, Watch>>>, event: DebouncedEvent) {
        for watch in watches.lock().unwrap().values() {
            watch.notify(&event)
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
        let mut watches = self.watches.lock().unwrap();

        match request {
            Request::Watch { id, root } => {
                if watches.contains_key(&id) {
                    emit_json(Response::Error {
                        id,
                        description: format!("Already registered a watch with id {}", id),
                    });
                } else {
                    match fs::canonicalize(root.as_path()) {
                        Ok(root) => {
                            match self.watcher.watch(root.as_path(), RecursiveMode::Recursive) {
                                Ok(()) => {
                                    watches.insert(id, Watch { id, root });
                                    emit_json(Response::Ok { id });
                                }
                                Err(error) => emit_json(Response::Error {
                                    id,
                                    description: error.description().to_string(),
                                }),
                            }
                        }
                        Err(error) => emit_json(Response::Error {
                            id,
                            description: error.description().to_string(),
                        }),
                    }
                }
            }
            Request::Unwatch { id } => {
                if let Some(watch) = watches.remove(&id) {
                    self.watcher.unwatch(watch.root.as_path()).unwrap();
                    emit_json(Response::Ok { id });
                } else {
                    emit_json(Response::Error {
                        id,
                        description: format!("No watch exists with id {}", id),
                    });
                }
            }
        }
    }
}

impl Watch {
    fn notify(&self, event: &DebouncedEvent) {
        match event {
            DebouncedEvent::Create(path) => {
                if path.starts_with(&self.root) {
                    emit_json(Event::Created {
                        watch_id: self.id,
                        path: path.clone(),
                    });
                }
            }
            DebouncedEvent::Write(path) => {
                if path.starts_with(&self.root) {
                    emit_json(Event::Modified {
                        watch_id: self.id,
                        path: path.clone(),
                    });
                }
            }
            DebouncedEvent::Remove(path) => {
                if path.starts_with(&self.root) {
                    emit_json(Event::Deleted {
                        watch_id: self.id,
                        path: path.clone(),
                    });
                }
            }
            DebouncedEvent::Rename(old_path, new_path) => {
                match (
                    old_path.starts_with(&self.root),
                    new_path.starts_with(&self.root),
                ) {
                    (true, true) => emit_json(Event::Renamed {
                        watch_id: self.id,
                        path: new_path.clone(),
                        old_path: old_path.clone(),
                    }),
                    (true, false) => emit_json(Event::Deleted {
                        watch_id: self.id,
                        path: old_path.clone(),
                    }),
                    (false, true) => emit_json(Event::Created {
                        watch_id: self.id,
                        path: new_path.clone(),
                    }),
                    (false, false) => {}
                }
            }
            DebouncedEvent::NoticeWrite(_path) => {}
            DebouncedEvent::NoticeRemove(_path) => {}
            DebouncedEvent::Chmod(_path) => {}
            DebouncedEvent::Rescan => {}
            DebouncedEvent::Error(_error, _path) => {}
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

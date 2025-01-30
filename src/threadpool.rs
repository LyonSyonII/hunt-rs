use std::{
    path::{Path, PathBuf},
    sync::{
        self,
        atomic::{AtomicBool, AtomicUsize},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
};

use thin_str::ThinStr;

use crate::{
    searchresult::{SearchResult, SearchResults},
    structs::{Output, Search},
};

type NoWork = Arc<(Mutex<()>, Condvar)>;
type WorkSender = crossbeam_channel::Sender<Option<Box<Path>>>;
type WorkReceiver = crossbeam_channel::Receiver<Option<Box<Path>>>;
type ResultSender = crossbeam_channel::Sender<crate::searchresult::SearchResult>;
type ResultReceiver = crossbeam_channel::Receiver<crate::searchresult::SearchResult>;

pub struct Pool {
    threads: Vec<JoinHandle<SearchResults>>,
    s_work: WorkSender,
    working: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,

    search: Search,
}

struct Worker {
    id: usize,

    s_work: WorkSender,
    r_work: WorkReceiver,
    working: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,

    results: SearchResults,
    search: Search,
}

impl Pool {
    pub fn new(search: Search) -> Self {
        let nthreads = std::thread::available_parallelism().unwrap().get();
        let mut threads = Vec::with_capacity(nthreads);
        let (s_work, r_work) = crossbeam_channel::bounded(nthreads);
        let working = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));

        for i in 0..nthreads {
            let (s_work, r_work) = (s_work.clone(), r_work.clone());
            let working = working.clone();
            let stop = stop.clone();
            let search = search.to_owned();
            threads.push(std::thread::spawn(move || {
                Worker::new(i, s_work, r_work, working, stop, search).work()
            }));
        }
        Self {
            threads,
            s_work,
            working,
            stop,
            search: search.to_owned(),
        }
    }

    pub fn send(&self, path: impl Into<Box<Path>>) {
        self.s_work.send(Some(path.into())).unwrap();
    }
    
    pub fn join(self) -> SearchResults {
        let mut results = SearchResults::with_capacity(8);
        for (i, thread) in self.threads.into_iter().enumerate() {
            results.merge(thread.join().unwrap());
            eprintln!("Joined Thread {i}");
        }
        results
    }

    pub fn stop(self) -> SearchResults {
        self.stop.store(true, sync::atomic::Ordering::Release);
        self.join()
    }
}

impl Worker {
    pub fn new(
        id: usize,
        s_work: WorkSender,
        r_work: WorkReceiver,
        working: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        search: Search,
    ) -> Worker {
        Self {
            id,
            results: SearchResults::with_capacity(8),
            s_work,
            r_work,
            working,
            stop,
            search,
        }
    }

    pub fn start_work(&self) {
        eprintln!("[{:02}] Starting work", self.id);
        self.working.fetch_add(1, sync::atomic::Ordering::AcqRel);
    }
    
    pub fn end_work(&self) {
        eprintln!("[{:02}] Finished work", self.id);
        self.working.fetch_sub(1, sync::atomic::Ordering::AcqRel);
    }

    pub fn work(mut self) -> SearchResults {
        let id: usize = self.id;
        loop {
            if self.stop.load(sync::atomic::Ordering::Acquire) {
                break;
            }
            eprintln!("[{id:02}] Waiting for more Work");
            match self.r_work.recv() {
                Ok(None) => {
                    eprintln!("[{id:02}] Received None, stopping");
                    break;
                },
                Ok(Some(path)) => {
                    self.start_work();
                    eprintln!("[{id:02}] Searching '{}'", path.display());
                    self.search_dir(path);
                    self.end_work();
                }
                Err(e) => panic!("{e}"),
            };
            if self.working.load(sync::atomic::Ordering::Relaxed) == 0 && self.r_work.is_empty() {
                eprintln!("[{id:02}] No more work, stopping all threads");
                for _ in 0..self.r_work.capacity().unwrap() {
                    self.s_work.send(None).unwrap();
                }
                break;
            }
        }
        self.results
    }

    pub fn send(&self, path: impl Into<Box<Path>>) {
        self.s_work.send(Some(path.into())).unwrap();
    }

    pub fn search_dir(&mut self, path: Box<Path>) {
        let read = {
            profi::prof!("search_dir::read_dir");
            let Ok(read) = std::fs::read_dir(&path) else {
                if self.search.verbose {
                    eprintln!("Could not read {:?}", path);
                }
                return;
            };
            read
        };

        profi::prof!("search_dir::inspect_entries");
        for entry in read.flatten() {
            profi::prof!("search_dir::inspect_entry");
            let Some((result, is_dir)) = crate::search::is_result(entry, &self.search) else {
                continue;
            };
            if let Some(result) = result {
                profi::prof!("search_dir::push_result");
                self.results.push(result);
            }
            if let Some(path) = is_dir {
                profi::prof!("search_dir::send_work");
                self.send(path)
            }
        }
    }
}

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

type WorkSender = crossbeam_channel::Sender<Option<Box<Path>>>;
type WorkReceiver = crossbeam_channel::Receiver<Option<Box<Path>>>;

pub struct Pool {
    threads: Vec<JoinHandle<SearchResults>>,
    s_work: WorkSender,
    working: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,

    search: Search,
}

struct Worker {
    id: usize,
    threads: usize,

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
        let (s_work, r_work) = crossbeam_channel::unbounded();
        let working = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        
        for i in 0..nthreads {
            let (s_work, r_work) = (s_work.clone(), r_work.clone());
            let working = working.clone();
            let stop = stop.clone();
            let search = search.to_owned();
            threads.push(std::thread::spawn(move || {
                Worker::new(i, nthreads, s_work, r_work, working, stop, search).work()
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
        for thread in self.threads.into_iter() {
            results.merge(thread.join().unwrap());
            // eprintln!("Joined Thread {i}");
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
        threads: usize,
        s_work: WorkSender,
        r_work: WorkReceiver,
        working: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        search: Search,
    ) -> Worker {
        Self {
            id,
            threads,
            results: SearchResults::with_capacity(8),
            s_work,
            r_work,
            working,
            stop,
            search,
        }
    }
    
    #[profi::profile]
    pub fn start_work(&self) {
        // eprintln!("[{:02}] Starting work", self.id);
        self.working.fetch_add(1, sync::atomic::Ordering::AcqRel);
    }
    
    #[profi::profile]
    pub fn end_work(&self) {
        // eprintln!("[{:02}] Finished work", self.id);
        self.working.fetch_sub(1, sync::atomic::Ordering::AcqRel);
    }
    
    #[profi::profile]
    pub fn work(mut self) -> SearchResults {
        let id: usize = self.id;
        loop {
            profi::prof!("work::loop");
            // eprintln!("[{id:02}] Waiting for more Work");
            match { profi::prof!("work::recv"); self.r_work.recv() } {
                Ok(None) => {
                    // eprintln!("[{id:02}] Received None, stopping");
                    break;
                },
                Ok(Some(path)) => {
                    profi::prof!("work::start_end");
                    self.start_work();
                    // eprintln!("[{id:02}] Searching '{}'", path.display());
                    self.search_dir(path);
                    self.end_work();
                }
                Err(e) => unreachable!("{e}"),
            };
            let should_stop = {
                profi::prof!("Worker::work::should_stop");
                self.working.load(sync::atomic::Ordering::Acquire) == 0 && self.r_work.is_empty()
            };
            if should_stop {
                // eprintln!("[{id:02}] No more work, stopping all threads");
                for _ in 0..self.threads-1 {
                    self.s_work.send(None).unwrap();
                }
                break;
            }
        }
        self.results
    }
    
    #[profi::profile]
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

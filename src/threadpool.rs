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

    search: Search,
}

struct Worker {
    id: usize,
    threads: usize,

    s_work: WorkSender,
    r_work: WorkReceiver,
    working: Arc<AtomicUsize>,

    results: SearchResults,
    search: Search,
}

impl Pool {
    pub fn new(search: Search) -> Self {
        let nthreads = std::thread::available_parallelism().unwrap().get();
        let mut threads = Vec::with_capacity(nthreads);
        let (s_work, r_work) = crossbeam_channel::unbounded();
        let working = Arc::new(AtomicUsize::new(0));

        for i in 0..nthreads {
            let (s_work, r_work) = (s_work.clone(), r_work.clone());
            let working = working.clone();
            let search = search.to_owned();
            threads.push(std::thread::spawn(move || {
                Worker::new(i, nthreads, s_work, r_work, working, search).work()
            }));
        }
        Self {
            threads,
            s_work,
            working,
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
        }
        results
    }
}

impl Worker {
    pub fn new(
        id: usize,
        threads: usize,
        s_work: WorkSender,
        r_work: WorkReceiver,
        working: Arc<AtomicUsize>,
        search: Search,
    ) -> Worker {
        Self {
            id,
            threads,
            results: SearchResults::with_capacity(8),
            s_work,
            r_work,
            working,
            search,
        }
    }

    #[profi::profile]
    pub fn start_work(&self) {
        self.working.fetch_add(1, sync::atomic::Ordering::AcqRel);
    }

    #[profi::profile]
    pub fn end_work(&self) {
        self.working.fetch_sub(1, sync::atomic::Ordering::AcqRel);
    }

    #[profi::profile]
    pub fn work(mut self) -> SearchResults {
        let id: usize = self.id;
        loop {
            match self.r_work.recv() {
                Ok(None) => break,
                Ok(Some(path)) => {
                    self.start_work();
                    self.search_dir(path);
                    self.end_work();
                }
                Err(e) => unreachable!("{e}"),
            };
            let should_stop =
                self.working.load(sync::atomic::Ordering::Acquire) == 0 && self.r_work.is_empty();

            if should_stop {
                for _ in 0..self.threads - 1 {
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

    #[profi::profile]
    pub fn search_dir(&mut self, path: Box<Path>) {
        let Ok(read) = std::fs::read_dir(&path) else {
            if self.search.verbose {
                eprintln!("Could not read {:?}", path);
            }
            return;
        };

        for entry in read.flatten() {
            let Some((result, is_dir)) = crate::search::is_result(entry, &self.search) else {
                continue;
            };
            if let Some(result) = result {
                self.results.push(result);
            }
            if let Some(path) = is_dir {
                self.send(path)
            }
        }
    }
}

//! Background job queue with priorities: warnings-related work first, routine data next,
//! housekeeping (thumbnails, log retention, cache trimming) last. Two worker threads, so
//! housekeeping never competes with the compositor for more than one core.

use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering},
        Condvar, Mutex, OnceLock,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
}

struct Job {
    priority: Priority,
    seq: u64,
    name: &'static str,
    work: Box<dyn FnOnce() + Send>,
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for Job {}
impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Job {
    // Higher priority first; within a priority, oldest first.
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority).then(other.seq.cmp(&self.seq))
    }
}

struct Queue {
    heap: Mutex<BinaryHeap<Job>>,
    ready: Condvar,
    seq: AtomicU64,
    running: AtomicUsize,
    done: AtomicU64,
}

fn queue() -> &'static Queue {
    static Q: OnceLock<Queue> = OnceLock::new();
    Q.get_or_init(|| {
        for i in 0..2 {
            let _ = std::thread::Builder::new().name(format!("jobs-{i}")).spawn(worker);
        }
        Queue { heap: Mutex::new(BinaryHeap::new()), ready: Condvar::new(), seq: AtomicU64::new(0), running: AtomicUsize::new(0), done: AtomicU64::new(0) }
    })
}

fn worker() {
    loop {
        let q = queue();
        let job = {
            let Ok(mut heap) = q.heap.lock() else { return };
            loop {
                if let Some(job) = heap.pop() {
                    break job;
                }
                heap = match q.ready.wait(heap) {
                    Ok(h) => h,
                    Err(_) => return,
                };
            }
        };
        q.running.fetch_add(1, AtomicOrdering::Relaxed);
        let name = job.name;
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job.work)).is_err() {
            log::warn!("background job \"{name}\" failed");
        }
        q.running.fetch_sub(1, AtomicOrdering::Relaxed);
        q.done.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

/// Queue work to run in the background.
pub fn submit(priority: Priority, name: &'static str, work: impl FnOnce() + Send + 'static) {
    let q = queue();
    let seq = q.seq.fetch_add(1, AtomicOrdering::Relaxed);
    if let Ok(mut heap) = q.heap.lock() {
        heap.push(Job { priority, seq, name, work: Box::new(work) });
    }
    q.ready.notify_one();
}

/// (waiting, running, finished) for the performance panel.
pub fn status() -> (usize, usize, u64) {
    let q = queue();
    let waiting = q.heap.lock().map(|h| h.len()).unwrap_or(0);
    (waiting, q.running.load(AtomicOrdering::Relaxed), q.done.load(AtomicOrdering::Relaxed))
}

//! Checking the calls caught in play on every core, with the results added
//! to the tallies exactly as checking them one by one, in order, would
//! (#45).
//!
//! Each call is checked in three steps (`capture`): it runs alone, giving
//! its key; a new one is compared with the rewrite, poisoned and varied;
//! and the first few from each caller get the scrambling check. Worker
//! threads do the steps. What depends on the calls before (which are
//! repeats, each one's case number, which get the scrambling check, and the
//! order of failures) is decided here, on the thread that caught them, by
//! three cursors that move along the calls in order:
//! 1. once a call has run, whether it's new, and its case number; a new one
//!    is sent on to be compared;
//! 2. once it has compared, whether it gets the scrambling check;
//! 3. once that is done, what it found is added to its tally.
//!
//! No step waits for more than the calls before it to reach the same
//! cursor, so the cores stay busy, and at most [`IN_FLIGHT`] calls are held
//! at once.

use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::Zx;

use crate::capture::{Compared, Routine, Run, SCRAMBLE_PER_CALLER, Scrambled, Tally, fail};

/// At most this many calls are held at once, each with a copy of the
/// machine or two. More only crowds the caches the thread playing the game
/// needs: measured, 512 is about the fastest (#45).
const IN_FLIGHT: usize = 512;

/// A call caught in play: which routine, the machine at its entry, and
/// where play was.
pub struct Job {
    pub routine: usize,
    pub entry: Zx,
    pub script: Script,
    pub frame: u32,
}

/// A step for a worker, and what it found.
enum Task {
    Run(usize, Arc<Job>),
    Compare(usize, Arc<Job>, Arc<Run>, String),
    Scramble(usize, Arc<Job>, Arc<Run>),
}

enum Found {
    /// A worker panicked: the run's panic.
    Panicked(Box<dyn std::any::Any + Send>),
    Ran(usize, Result<Arc<Run>, String>),
    Compared(usize, Compared),
    Scrambled(usize, Scrambled),
}

/// Where a call is in its checking.
enum Step {
    /// Waiting to run, or to be decided on once it has.
    Running(Option<Result<Arc<Run>, String>>),
    /// Repeats and calls that read the ROM: nothing more to do.
    Settled,
    /// It didn't return: a failure, added in turn.
    Failed(String),
    /// New, and being compared.
    Comparing(Arc<Run>, String, Option<Compared>),
    /// Compared, and being scrambled if it gets the check.
    Scrambling(Arc<Run>, String, Compared, Option<Option<Scrambled>>),
}

struct Call {
    job: Arc<Job>,
    step: Step,
}

pub struct Checker {
    routines: Arc<[Routine]>,
    tallies: Vec<Tally>,
    /// The calls not yet added to their tallies, the first numbered `first`.
    calls: VecDeque<Call>,
    first: usize,
    /// How far the second and third cursors have got; the first is the
    /// first call still `Running`.
    decided: usize,
    compared: usize,
    /// The scrambling checks decided on, per routine and caller.
    scrambling: Vec<BTreeMap<u16, u32>>,
    tasks: Option<Sender<Task>>,
    found: Receiver<Found>,
    workers: Vec<JoinHandle<()>>,
}

impl Checker {
    pub fn new(routines: Arc<[Routine]>, assets: &Arc<Assets>) -> Checker {
        let (tasks, queue) = channel::<Task>();
        let (report, found) = channel();
        let queue = Arc::new(Mutex::new(queue));
        let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let workers = (0..threads)
            .map(|_| {
                let (queue, report) = (Arc::clone(&queue), report.clone());
                let (routines, assets) = (Arc::clone(&routines), Arc::clone(assets));
                std::thread::spawn(move || {
                    loop {
                        let task = queue.lock().expect("a worker panicked").recv();
                        let Ok(task) = task else { return };
                        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            work(&routines, &assets, task)
                        }))
                        .unwrap_or_else(Found::Panicked);
                        if report.send(done).is_err() {
                            return;
                        }
                    }
                })
            })
            .collect();
        Checker {
            tallies: routines.iter().map(|_| Tally::default()).collect(),
            scrambling: routines.iter().map(|_| BTreeMap::new()).collect(),
            routines,
            calls: VecDeque::new(),
            first: 0,
            decided: 0,
            compared: 0,
            tasks: Some(tasks),
            found,
            workers,
        }
    }

    /// Takes a call to be checked. Waits first if too many are held.
    pub fn take(&mut self, job: Job) {
        while self.calls.len() >= IN_FLIGHT {
            self.wait();
        }
        let n = self.first + self.calls.len();
        let job = Arc::new(job);
        send(&self.tasks, Task::Run(n, Arc::clone(&job)));
        self.calls.push_back(Call {
            job,
            step: Step::Running(None),
        });
        while let Ok(found) = self.found.try_recv() {
            self.record(found);
        }
    }

    /// Waits until every call taken is in its tally, and returns them.
    pub fn tallies(&mut self) -> &[Tally] {
        while !self.calls.is_empty() {
            self.wait();
        }
        &self.tallies
    }

    /// Waits until every call taken is in its tally, and hands the tallies
    /// over.
    #[cfg(test)]
    pub fn finish(mut self) -> Vec<Tally> {
        self.tallies();
        std::mem::take(&mut self.tallies)
    }

    fn wait(&mut self) {
        let found = self.found.recv().expect("the workers are running");
        self.record(found);
    }

    /// Notes what a worker found, and moves the cursors as far as they go.
    fn record(&mut self, found: Found) {
        let first = self.first;
        match found {
            Found::Panicked(e) => std::panic::resume_unwind(e),
            Found::Ran(n, run) => self.calls[n - first].step = Step::Running(Some(run)),
            Found::Compared(n, c) => {
                if let Step::Comparing(_, _, done) = &mut self.calls[n - first].step {
                    *done = Some(c);
                }
            }
            Found::Scrambled(n, s) => {
                if let Step::Scrambling(_, _, _, done) = &mut self.calls[n - first].step {
                    *done = Some(Some(s));
                }
            }
        }
        self.decide();
        self.choose_scrambles();
        self.add();
    }

    /// The first cursor: which calls are new, and their case numbers.
    fn decide(&mut self) {
        while self.decided < self.first + self.calls.len() {
            let n = self.decided;
            let call = &mut self.calls[n - self.first];
            let Step::Running(ran) = &mut call.step else {
                unreachable!("the first cursor is at a call still running")
            };
            let Some(ran) = ran.take() else { return };
            let job = Arc::clone(&call.job);
            let (r, t) = (&self.routines[job.routine], &mut self.tallies[job.routine]);
            t.calls += 1;
            let step = match ran {
                Err(e) => Step::Failed(e),
                Ok(run) if !t.seen.insert(run.key) => {
                    t.repeats += 1;
                    Step::Settled
                }
                Ok(run) if run.rom_read.is_some() => {
                    let read = run.rom_read.expect("just matched");
                    t.rom_reads += 1;
                    t.first_rom_read.get_or_insert(format!(
                        "the instruction at {:04x} read {:04x}, in a call from {:04x}",
                        read.0,
                        read.1,
                        run.ret.wrapping_sub(3)
                    ));
                    Step::Settled
                }
                Ok(run) => {
                    t.compared += 1;
                    t.executed.extend(run.executed.iter().copied());
                    let at = r.at(&run, t.compared);
                    send(
                        &self.tasks,
                        Task::Compare(n, job, Arc::clone(&run), at.clone()),
                    );
                    Step::Comparing(run, at, None)
                }
            };
            self.calls[n - self.first].step = step;
            self.decided += 1;
        }
    }

    /// The second cursor: which compared clean and get the scrambling check:
    /// the first [`SCRAMBLE_PER_CALLER`] from each caller.
    fn choose_scrambles(&mut self) {
        while self.compared < self.decided {
            let call = &mut self.calls[self.compared - self.first];
            let job = Arc::clone(&call.job);
            if let Step::Comparing(run, at, c) = &mut call.step {
                let Some(c) = c.take() else { return };
                let (run, at) = (Arc::clone(run), std::mem::take(at));
                let scrambles =
                    c.failure.is_none() && self.routines[job.routine].scrambles_anything() && {
                        let count = self.scrambling[job.routine]
                            .entry(self.routines[job.routine].caller(&run))
                            .or_default();
                        *count < SCRAMBLE_PER_CALLER && {
                            *count += 1;
                            true
                        }
                    };
                let done = if scrambles {
                    send(
                        &self.tasks,
                        Task::Scramble(self.compared, job, Arc::clone(&run)),
                    );
                    None
                } else {
                    Some(None)
                };
                call.step = Step::Scrambling(run, at, c, done);
            }
            self.compared += 1;
        }
    }

    /// The third cursor: what each call found, added to its tally in order.
    fn add(&mut self) {
        while self.first < self.compared {
            let call = &mut self.calls[0];
            let (r, t) = (
                &self.routines[call.job.routine],
                &mut self.tallies[call.job.routine],
            );
            match &mut call.step {
                Step::Running(_) | Step::Comparing(..) => {
                    unreachable!("the third cursor is behind the second")
                }
                Step::Settled => {}
                Step::Failed(e) => fail(t, std::mem::take(e)),
                Step::Scrambling(run, at, c, scrambled) => {
                    let Some(scrambled) = scrambled.take() else {
                        return;
                    };
                    t.rom_reads += c.rom_reads;
                    t.varied += c.varied;
                    t.varied_hung += c.varied_hung;
                    if let Some(e) = c.failure.take() {
                        fail(t, e);
                    } else if let Some(s) = scrambled {
                        *t.scrambled_by_caller.entry(r.caller(run)).or_default() += 1;
                        t.scrambled += 1;
                        match s {
                            Scrambled::Reads(e) => fail(t, format!("{}: {at}: {e}", r.name)),
                            Scrambled::UnreadStackLeftOut => t.stack_left_out += 1,
                            Scrambled::Unread => {}
                        }
                    }
                }
            }
            self.calls.pop_front();
            self.first += 1;
        }
    }
}

impl Drop for Checker {
    fn drop(&mut self) {
        // Closing the queue lets the workers finish.
        self.tasks = None;
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}

fn send(tasks: &Option<Sender<Task>>, task: Task) {
    tasks
        .as_ref()
        .expect("the workers are running")
        .send(task)
        .expect("the workers are running");
}

/// One step of a call's check, on a worker.
fn work(routines: &[Routine], assets: &Assets, task: Task) -> Found {
    match task {
        Task::Run(n, job) => Found::Ran(
            n,
            routines[job.routine].run_original(&job.entry).map(Arc::new),
        ),
        Task::Compare(n, job, run, at) => Found::Compared(
            n,
            routines[job.routine].compare_all(&job.entry, &run, &job.play(assets), &at),
        ),
        Task::Scramble(n, job, run) => {
            let r = &routines[job.routine];
            Found::Scrambled(n, r.scramble(&run.after, r.level(&run), &job.play(assets)))
        }
    }
}

impl Job {
    fn play<'a>(&'a self, assets: &'a Assets) -> crate::capture::Play<'a> {
        crate::capture::Play {
            script: &self.script,
            frame: self.frame,
            assets,
        }
    }
}

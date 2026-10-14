//! Runtime/allocator-owned reclamation. Domain crates only name the boundary.

pub(crate) use active::NapiHeapReclaimer;

impl NapiHeapReclaimer {
  pub(crate) fn configure_runtime(builder: &mut tokio::runtime::Builder, workers: usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    const WORKER_NAME: &str = "arkdown-rolldown-worker";
    let next_thread = AtomicUsize::new(0);
    builder
      // Tokio's multi_thread::Launch starts all scheduler workers before build()
      // returns. blocking::Pool also invokes on_thread_start, so distinguish the
      // initial worker threads from subsequently created blocking-pool threads.
      .thread_name_fn(move || {
        if next_thread.fetch_add(1, Ordering::Relaxed) < workers {
          WORKER_NAME.into()
        } else {
          "arkdown-rolldown-blocking".into()
        }
      })
      .on_thread_start(|| Self::thread_started(std::thread::current().name() == Some(WORKER_NAME)))
      .on_thread_park(Self::collect_epoch)
      .on_thread_unpark(Self::collect_on_unpark);
  }
}

#[cfg(all(
  feature = "allocator",
  not(debug_assertions),
  not(feature = "dhat-heap"),
  not(any(
    target_arch = "arm",
    target_os = "android",
    target_os = "freebsd",
    target_os = "windows",
    target_family = "wasm"
  ))
))]
mod active {
  use arkdown_build::request::{BuildHeapBoundary, BuildHeapReclaimer};
  use std::{
    cell::Cell,
    collections::VecDeque,
    sync::{
      atomic::{AtomicU64, Ordering},
      Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
  };

  const WORKER_HOLD_TIMEOUT: Duration = Duration::from_millis(20);

  // The allocator and N-API runtime are process-global; domain crates only hold
  // the injected BuildHeapReclaimer. No per-build thread registry is retained.
  static RECLAIMER: RuntimeReclaimer = RuntimeReclaimer {
    epoch: AtomicU64::new(0),
    coverage: Mutex::new(VecDeque::new()),
    changed: Condvar::new(),
  };
  thread_local! {
    static LAST_SEEN: Cell<u64> = const { Cell::new(0) };
    static IS_WORKER: Cell<bool> = const { Cell::new(false) };
  }

  pub(crate) struct NapiHeapReclaimer {
    main_thread:
      napi::threadsafe_function::ThreadsafeFunction<(), (), (), napi::Status, false, false>,
  }

  impl std::fmt::Debug for NapiHeapReclaimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("NapiHeapReclaimer").finish_non_exhaustive()
    }
  }

  impl NapiHeapReclaimer {
    pub(crate) fn from_env(env: &napi::Env) -> napi::Result<Arc<dyn BuildHeapReclaimer>> {
      let callback = env.create_function_from_closure::<(), (), _>("reclaimBuildHeap", |_| {
        Self::collect_epoch();
        Ok(())
      })?;
      let main_thread = callback
        .build_threadsafe_function::<()>()
        .build_callback(|_| Ok(()))?;
      Ok(Arc::new(Self { main_thread }))
    }

    pub(super) fn thread_started(is_worker: bool) {
      IS_WORKER.set(is_worker);
    }

    pub(crate) fn collect_epoch() {
      RECLAIMER.collect();
    }

    pub(super) fn collect_on_unpark() {
      RECLAIMER.collect_on_unpark();
    }
  }

  impl BuildHeapReclaimer for NapiHeapReclaimer {
    fn reclaim(&self, boundary: BuildHeapBoundary) {
      RECLAIMER.reclaim(boundary);
      // mimalloc heap.c::mi_heap_collect_ex only reclaims all abandoned
      // segments on its main thread. tsrs scoped checkers have already exited.
      // Schedule this through N-API without waiting for JavaScript or workers.
      let status = self.main_thread.call(
        (),
        napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking,
      );
      if status != napi::Status::Ok {
        tracing::debug!(?status, "main-thread heap reclamation was not scheduled");
      }
    }
  }

  struct RuntimeReclaimer {
    epoch: AtomicU64,
    coverage: Mutex<VecDeque<EpochCoverage>>,
    changed: Condvar,
  }

  struct EpochCoverage {
    epoch: u64,
    boundary: BuildHeapBoundary,
    workers: usize,
    collected_workers: usize,
    collecting_workers: usize,
    started: Instant,
    footprint_before: Option<u64>,
    timeout_logged: bool,
    wake_deadline: Instant,
  }

  impl EpochCoverage {
    fn log(&self, outcome: &str) {
      tracing::debug!(boundary = ?self.boundary, epoch = self.epoch,
        workers_covered = self.collected_workers, workers = self.workers,
        workers_collecting = self.collecting_workers,
        elapsed_us = self.started.elapsed().as_micros(), outcome,
        footprint_before = ?self.footprint_before,
        footprint_after = ?RuntimeReclaimer::footprint(), "Tokio heap reclaim coverage");
    }
  }

  impl RuntimeReclaimer {
    fn collect(&self) {
      let epoch = self.epoch.load(Ordering::Acquire);
      let last_epoch = LAST_SEEN.replace(epoch);
      if last_epoch == epoch {
        return;
      }
      if IS_WORKER.get() {
        let mut guard = self
          .coverage
          .lock()
          .expect("heap reclaim coverage poisoned");
        for coverage in guard
          .iter_mut()
          .filter(|s| last_epoch < s.epoch && s.epoch <= epoch)
        {
          coverage.collecting_workers += 1;
        }
      }
      let before = Self::footprint();
      let started = Instant::now();
      arkdown_ets_transform::transform::source_emit::HvigorTypescriptPrinter::release_thread_scratch();
      // SAFETY: mimalloc is the global allocator under this module's cfg.
      // mi_collect operates on the calling thread's heap; no shared heap
      // handle or live allocation is accessed by the caller.
      unsafe { libmimalloc_sys2::mi_collect(true) };
      tracing::debug!(epoch, thread = ?std::thread::current().id(), name = ?std::thread::current().name(), elapsed_us = started.elapsed().as_micros(), footprint_before = ?before, footprint_after = ?Self::footprint(), "worker heap reclaimed");
      if IS_WORKER.get() {
        let mut guard = self
          .coverage
          .lock()
          .expect("heap reclaim coverage poisoned");
        // A collect for a newer boundary also covers any skipped older epoch.
        // Keep in-flight boundary counts until they finish; the bundle and ABC
        // requests can overlap without making the build caller wait.
        for coverage in guard
          .iter_mut()
          .filter(|s| last_epoch < s.epoch && s.epoch <= epoch)
        {
          coverage.collecting_workers -= 1;
          coverage.collected_workers += 1;
          if coverage.collected_workers == coverage.workers {
            coverage.log("complete");
            self.changed.notify_all();
          }
        }
      }
    }

    fn collect_on_unpark(&self) {
      let epoch = self.epoch.load(Ordering::Acquire);
      if LAST_SEEN.get() == epoch {
        return;
      }
      {
        let guard = self
          .coverage
          .lock()
          .expect("heap reclaim coverage poisoned");
        if guard
          .back()
          .is_some_and(|s| s.epoch == epoch && Instant::now() < s.wake_deadline)
        {
          // Tokio worker::park calls after_unpark while still searching. An
          // inline collect here serializes wakeups: the next sleeper is only
          // notified when run_task transitions out of searching. Let the wake
          // task collect during its bounded window; keep this hook as backstop.
          return;
        }
      }
      self.collect();
    }

    fn wake_worker(&self, epoch: u64, deadline: Instant) {
      if self.epoch.load(Ordering::Acquire) != epoch {
        return;
      }
      self.collect();
      let mut guard = self
        .coverage
        .lock()
        .expect("heap reclaim coverage poisoned");
      while let Some(coverage) = guard.back_mut().filter(|state| state.epoch == epoch) {
        if coverage.collected_workers == coverage.workers {
          return;
        }
        let now = Instant::now();
        if now >= deadline {
          for pending in guard.iter_mut().filter(|s| s.collected_workers < s.workers) {
            if !pending.timeout_logged {
              pending.timeout_logged = true;
              pending.log("timeout");
            }
          }
          return;
        }
        // probe1's barrier deadlocked on the worker running the build future.
        // That caller counts itself synchronously and never waits here. Holds
        // on wake tasks expire even if another worker stays in synchronous work.
        guard = self
          .changed
          .wait_timeout(guard, deadline - now)
          .expect("heap reclaim coverage poisoned")
          .0;
      }
    }

    fn reclaim(&'static self, boundary: BuildHeapBoundary) {
      let before = Self::footprint();
      let started = Instant::now();
      let runtime = tokio::runtime::Handle::try_current().ok();
      let workers = runtime.as_ref().map_or(0, |rt| rt.metrics().num_workers());
      let epoch = {
        let mut guard = self
          .coverage
          .lock()
          .expect("heap reclaim coverage poisoned");
        guard.retain_mut(|previous| {
          if previous.collected_workers == previous.workers {
            return false;
          }
          if Instant::now() >= previous.wake_deadline && !previous.timeout_logged {
            previous.log("timeout");
            previous.timeout_logged = true;
          }
          // A timed-out hold does not cancel mi_collect already running. Keep
          // its completion evidence even when the next boundary has started.
          !previous.timeout_logged || previous.collecting_workers != 0
        });
        let epoch = self.epoch.load(Ordering::Relaxed) + 1;
        guard.push_back(EpochCoverage {
          epoch,
          boundary,
          workers,
          collected_workers: 0,
          collecting_workers: 0,
          started,
          footprint_before: before,
          timeout_logged: false,
          wake_deadline: started + WORKER_HOLD_TIMEOUT,
        });
        self.epoch.store(epoch, Ordering::Release);
        self.changed.notify_all();
        epoch
      };
      self.collect();
      rayon::broadcast(|_| self.collect());
      if let Some(runtime) = runtime {
        // A Rayon submission enters Tokio's shared injection queue, not the
        // build worker's LIFO slot. The caller never waits for worker coverage.
        rayon::spawn(move || {
          let deadline = Instant::now() + WORKER_HOLD_TIMEOUT;
          {
            let mut guard = self
              .coverage
              .lock()
              .expect("heap reclaim coverage poisoned");
            let Some(coverage) = guard.back_mut().filter(|s| s.epoch == epoch) else {
              return;
            };
            coverage.wake_deadline = deadline;
          }
          for _ in 0..workers {
            runtime.spawn(async move { self.wake_worker(epoch, deadline) });
          }
        });
      }
      tracing::debug!(?boundary, epoch, elapsed_us = started.elapsed().as_micros(),
        footprint_before = ?before, footprint_after = ?Self::footprint(),
        "build heap reclaimed (Tokio workers collect asynchronously)");
    }

    #[cfg(target_os = "macos")]
    fn footprint() -> Option<u64> {
      let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v0>::zeroed();
      // SAFETY: RUSAGE_INFO_V0 writes exactly rusage_info_v0 to the provided
      // aligned buffer. Read it only after a successful call for this process.
      let result = unsafe {
        libc::proc_pid_rusage(
          std::process::id() as i32,
          libc::RUSAGE_INFO_V0,
          usage.as_mut_ptr().cast(),
        )
      };
      (result == 0).then(|| unsafe { usage.assume_init().ri_phys_footprint })
    }

    #[cfg(not(target_os = "macos"))]
    fn footprint() -> Option<u64> {
      // phys_footprint is a macOS metric. Do not label RSS as footprint.
      None
    }
  }
}

#[cfg(not(all(
  feature = "allocator",
  not(debug_assertions),
  not(feature = "dhat-heap"),
  not(any(
    target_arch = "arm",
    target_os = "android",
    target_os = "freebsd",
    target_os = "windows",
    target_family = "wasm"
  ))
)))]
mod active {
  use arkdown_build::request::{BuildHeapBoundary, BuildHeapReclaimer};

  #[derive(Debug)]
  pub(crate) struct NapiHeapReclaimer;

  impl NapiHeapReclaimer {
    pub(crate) fn from_env(_: &napi::Env) -> napi::Result<std::sync::Arc<dyn BuildHeapReclaimer>> {
      Ok(std::sync::Arc::new(Self))
    }

    pub(super) fn thread_started(_: bool) {}

    pub(crate) fn collect_epoch() {}

    pub(super) fn collect_on_unpark() {}
  }

  impl BuildHeapReclaimer for NapiHeapReclaimer {
    fn reclaim(&self, _: BuildHeapBoundary) {}
  }
}

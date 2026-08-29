use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use napi_derive::napi;

#[derive(Clone)]
struct RuntimeHvigorWorkRecord {
  priority: i32,
  sequence: u64,
  candidate_workers: Vec<u32>,
  state: String,
  worker_id: Option<u32>,
  start_time: Option<f64>,
  end_time: Option<f64>,
}

#[napi(object)]
pub struct RuntimeHvigorWorkerSubmission {
  pub accepted: bool,
  pub work_id: String,
  pub candidate_workers: Vec<u32>,
  pub submit_time: f64,
}

#[napi(object)]
pub struct RuntimeHvigorWorkerControlSnapshot {
  pub state: String,
  pub worker_id: Option<u32>,
  pub start_time: Option<f64>,
  pub end_time: Option<f64>,
}

/// Rust-owned priority queue and lifecycle state for the public Hvigor worker
/// pool. Node worker threads execute JavaScript modules because those modules
/// are V8 values; acceptance, targeting, ordering, IDs, and transitions stay
/// native and are shared by all delegator facades for one runtime host.
#[napi]
pub struct RuntimeHvigorWorkerPoolState {
  max_pool_num: u32,
  min_pool_num: u32,
  next_work_id: u64,
  next_sequence: u64,
  state: String,
  resident: bool,
  works: IndexMap<String, RuntimeHvigorWorkRecord>,
  queue: Vec<String>,
}

#[napi]
impl RuntimeHvigorWorkerPoolState {
  #[napi(constructor)]
  pub fn new(max_pool_num: Option<u32>) -> Self {
    let available = std::thread::available_parallelism()
      .map(|value| value.get().saturating_sub(1) as u32)
      .unwrap_or(1);
    let max_pool_num = max_pool_num.unwrap_or(available).max(1);
    Self {
      max_pool_num,
      min_pool_num: max_pool_num.min(2),
      next_work_id: 0,
      next_sequence: 0,
      state: "running".to_string(),
      resident: true,
      works: IndexMap::new(),
      queue: Vec::new(),
    }
  }

  #[napi]
  pub fn submit(
    &mut self,
    parallel_execution: bool,
    priority: i32,
    target_workers: Option<Vec<i32>>,
  ) -> RuntimeHvigorWorkerSubmission {
    let work_id = self.next_work_id.to_string();
    self.next_work_id = self.next_work_id.saturating_add(1);
    let candidates = self.requested_candidate_workers(target_workers);
    let accepted = parallel_execution && self.state == "running" && !candidates.is_empty();
    let record = RuntimeHvigorWorkRecord {
      priority,
      sequence: self.next_sequence,
      candidate_workers: candidates.clone(),
      state: if accepted { "waiting" } else { "reject" }.to_string(),
      worker_id: None,
      start_time: None,
      end_time: None,
    };
    self.next_sequence = self.next_sequence.saturating_add(1);
    self.works.insert(work_id.clone(), record);
    if accepted {
      self.queue.push(work_id.clone());
      self.sort_queue();
    }
    RuntimeHvigorWorkerSubmission {
      accepted,
      work_id,
      candidate_workers: candidates,
      submit_time: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1_000.0,
    }
  }

  #[napi]
  pub fn take_for_worker(&mut self, worker_id: u32, start_time: f64) -> Option<String> {
    let index = self.queue.iter().position(|work_id| {
      self
        .works
        .get(work_id)
        .is_some_and(|record| record.candidate_workers.contains(&worker_id))
    })?;
    let work_id = self.queue.remove(index);
    if let Some(record) = self.works.get_mut(&work_id) {
      record.state = "running".to_string();
      record.worker_id = Some(worker_id);
      record.start_time = Some(start_time);
    }
    Some(work_id)
  }

  #[napi]
  pub fn finish(&mut self, work_id: String, succeeded: bool, end_time: f64) {
    if let Some(record) = self.works.get_mut(&work_id) {
      record.state = if succeeded { "end" } else { "error" }.to_string();
      record.end_time = Some(end_time);
    }
  }

  #[napi]
  pub fn set_work_priority(&mut self, work_id: String, priority: i32) -> bool {
    let Some(record) = self.works.get_mut(&work_id) else {
      return false;
    };
    if record.state != "waiting" {
      return false;
    }
    record.priority = priority;
    self.sort_queue();
    true
  }

  #[napi]
  pub fn set_work_state(&mut self, work_id: String, next: String) -> bool {
    let Some(record) = self.works.get_mut(&work_id) else {
      return false;
    };
    let allowed = match record.state.as_str() {
      "waiting" => matches!(next.as_str(), "running" | "reject"),
      "running" => matches!(next.as_str(), "end" | "error"),
      _ => false,
    };
    if allowed {
      record.state = next;
    }
    allowed
  }

  #[napi]
  pub fn set_work_worker_id(&mut self, work_id: String, worker_id: u32) {
    if let Some(record) = self.works.get_mut(&work_id) {
      record.worker_id = Some(worker_id);
    }
  }

  #[napi]
  pub fn set_work_start_time(&mut self, work_id: String, start_time: f64) {
    if let Some(record) = self.works.get_mut(&work_id) {
      record.start_time = Some(start_time);
    }
  }

  #[napi]
  pub fn set_work_end_time(&mut self, work_id: String, end_time: f64) {
    if let Some(record) = self.works.get_mut(&work_id) {
      record.end_time = Some(end_time);
    }
  }

  #[napi]
  pub fn snapshot(&self, work_id: String) -> RuntimeHvigorWorkerControlSnapshot {
    let record = self.works.get(&work_id);
    RuntimeHvigorWorkerControlSnapshot {
      state: record
        .map(|record| record.state.clone())
        .unwrap_or_else(|| "reject".to_string()),
      worker_id: record.and_then(|record| record.worker_id),
      start_time: record.and_then(|record| record.start_time),
      end_time: record.and_then(|record| record.end_time),
    }
  }

  #[napi]
  pub fn candidate_workers(&self, work_id: String) -> Vec<u32> {
    self
      .works
      .get(&work_id)
      .map(|record| record.candidate_workers.clone())
      .unwrap_or_default()
  }

  #[napi]
  pub fn has_queued_work(&self) -> bool {
    !self.queue.is_empty()
  }

  #[napi]
  pub fn set_state(&mut self, next: String) {
    let allowed = match self.state.as_str() {
      "init" => matches!(next.as_str(), "running"),
      "running" => matches!(next.as_str(), "close" | "stop"),
      "close" => matches!(next.as_str(), "running" | "terminated"),
      "stop" => matches!(next.as_str(), "terminated"),
      _ => false,
    };
    if allowed {
      self.state = next;
      if self.state == "stop" {
        self.reject_queue();
      }
    }
  }

  #[napi]
  pub fn terminate(&mut self) {
    self.reject_queue();
    self.state = "terminated".to_string();
  }

  #[napi]
  pub fn reset(&mut self) {
    self.works.clear();
    self.queue.clear();
    self.next_work_id = 0;
    self.next_sequence = 0;
    self.state = "running".to_string();
  }

  #[napi(getter)]
  pub fn state(&self) -> String {
    self.state.clone()
  }

  #[napi(getter)]
  pub fn max_pool_num(&self) -> u32 {
    self.max_pool_num
  }

  #[napi(getter)]
  pub fn min_pool_num(&self) -> u32 {
    self.min_pool_num
  }

  #[napi(getter)]
  pub fn resident(&self) -> bool {
    self.resident
  }

  #[napi(setter)]
  pub fn set_resident(&mut self, resident: bool) {
    self.resident = resident;
  }
}

impl RuntimeHvigorWorkerPoolState {
  fn requested_candidate_workers(&self, requested: Option<Vec<i32>>) -> Vec<u32> {
    let Some(requested) = requested else {
      return (0..self.max_pool_num).collect();
    };
    let mut seen = HashSet::new();
    requested
      .into_iter()
      .filter_map(|value| u32::try_from(value).ok())
      .filter(|value| *value < self.max_pool_num && seen.insert(*value))
      .collect()
  }

  fn sort_queue(&mut self) {
    let works = &self.works;
    self.queue.sort_by_key(|work_id| {
      works
        .get(work_id)
        .map(|record| (record.priority, record.sequence))
        .unwrap_or((i32::MAX, u64::MAX))
    });
  }

  fn reject_queue(&mut self) {
    for work_id in self.queue.drain(..) {
      if let Some(record) = self.works.get_mut(&work_id) {
        record.state = "reject".to_string();
      }
    }
  }
}

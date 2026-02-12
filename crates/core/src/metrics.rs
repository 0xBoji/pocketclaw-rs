use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub uptime_secs: u64,
    pub messages_in: u64,
    pub messages_out: u64,
    pub tool_calls: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
}

#[derive(Debug)]
pub struct MetricsStore {
    start_time: Instant,
    messages_in: AtomicU64,
    messages_out: AtomicU64,
    tool_calls: AtomicU64,
    tokens_input: AtomicU64,
    tokens_output: AtomicU64,
}

impl MetricsStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            start_time: Instant::now(),
            messages_in: AtomicU64::new(0),
            messages_out: AtomicU64::new(0),
            tool_calls: AtomicU64::new(0),
            tokens_input: AtomicU64::new(0),
            tokens_output: AtomicU64::new(0),
        })
    }

    pub fn inc_messages_in(&self) {
        self.messages_in.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_messages_out(&self) {
        self.messages_out.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_tool_calls(&self) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_tokens(&self, input: u64, output: u64) {
        self.tokens_input.fetch_add(input, Ordering::Relaxed);
        self.tokens_output.fetch_add(output, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            uptime_secs: self.start_time.elapsed().as_secs(),
            messages_in: self.messages_in.load(Ordering::Relaxed),
            messages_out: self.messages_out.load(Ordering::Relaxed),
            tool_calls: self.tool_calls.load(Ordering::Relaxed),
            tokens_input: self.tokens_input.load(Ordering::Relaxed),
            tokens_output: self.tokens_output.load(Ordering::Relaxed),
        }
    }
}

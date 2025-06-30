use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use serde::Serialize;
use chrono::Utc;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: i64,
    pub level: String,
    pub message: String,
    pub directory: Option<String>,
}

#[derive(Clone)]
pub struct LogBuffer {
    buffer: Arc<Mutex<VecDeque<LogEntry>>>,
    max_entries: usize,
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(max_entries))),
            max_entries,
        }
    }

    pub fn add_log(&self, level: &str, message: String, directory: Option<String>) {
        let entry = LogEntry {
            timestamp: Utc::now().timestamp(),
            level: level.to_string(),
            message,
            directory,
        };

        if let Ok(mut buffer) = self.buffer.lock() {
            if buffer.len() >= self.max_entries {
                buffer.pop_front();
            }
            buffer.push_back(entry);
        }
    }

    pub fn get_logs(&self, limit: Option<usize>) -> Vec<LogEntry> {
        if let Ok(buffer) = self.buffer.lock() {
            let logs: Vec<LogEntry> = buffer.iter().cloned().collect();
            match limit {
                Some(n) => logs.into_iter().rev().take(n).rev().collect(),
                None => logs,
            }
        } else {
            Vec::new()
        }
    }

    pub fn clear(&self) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.clear();
        }
    }
}
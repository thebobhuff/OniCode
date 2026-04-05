use std::{collections::VecDeque, sync::Arc};

use parking_lot::Mutex;

use crate::message::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum MessagePriority {
    Steering,
    FollowUp,
}

#[derive(Debug)]
pub struct QueuedMessage {
    pub message: Message,
    pub priority: MessagePriority,
}

#[derive(Clone)]
pub struct MessageQueue {
    queue: Arc<Mutex<VecDeque<QueuedMessage>>>,
    mode: Arc<Mutex<QueueMode>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueueMode {
    All,
    OneAtATime,
}

impl MessageQueue {
    pub fn new(mode: QueueMode) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            mode: Arc::new(Mutex::new(mode)),
        }
    }

    pub fn enqueue(&self, message: Message, priority: MessagePriority) {
        self.queue
            .lock()
            .push_back(QueuedMessage { message, priority });
    }

    pub fn drain_steering(&self) -> Vec<Message> {
        let mut q = self.queue.lock();
        let mut result = Vec::new();

        while let Some(idx) = q
            .iter()
            .position(|m| m.priority == MessagePriority::Steering)
        {
            result.push(q.remove(idx).unwrap().message);
        }

        result
    }

    pub fn drain_followup(&self) -> Vec<Message> {
        let mut q = self.queue.lock();
        let mode = self.mode.lock().clone();

        match mode {
            QueueMode::All => q.drain(..).map(|m| m.message).collect(),
            QueueMode::OneAtATime => {
                if let Some(item) = q.pop_front() {
                    vec![item.message]
                } else {
                    Vec::new()
                }
            }
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.queue.lock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    pub fn clear(&self) {
        self.queue.lock().clear();
    }

    pub fn set_mode(&self, mode: QueueMode) {
        *self.mode.lock() = mode;
    }
}

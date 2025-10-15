use std::collections::VecDeque;

use malachitebft_core_types::Context;
use tracing::{info, warn};

use crate::consensus::ConsensusMsg;

pub struct MessageBuffer<Ctx: Context> {
    messages: VecDeque<ConsensusMsg<Ctx>>,
    max_size: usize,
}

impl<Ctx: Context> MessageBuffer<Ctx> {
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: VecDeque::new(),
            max_size,
        }
    }

    pub fn buffer(&mut self, msg: ConsensusMsg<Ctx>) -> bool {
        if self.messages.len() < self.max_size {
            info!("Buffering message: {msg:?}");
            self.messages.push_back(msg);
            true
        } else {
            warn!("Buffer is full, dropping message: {msg:?}");
            false
        }
    }

    pub fn pop(&mut self) -> Option<ConsensusMsg<Ctx>> {
        self.messages.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}

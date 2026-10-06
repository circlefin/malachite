use std::collections::VecDeque;
use std::fmt;

use tracing::info;

/// The buffer was already at capacity, and the message was not stored.
/// Carries the rejected message back to the caller.
#[derive(Debug)]
pub struct BufferFull<T>(pub T);

pub struct MessageBuffer<T> {
    messages: VecDeque<T>,
    max_size: usize,
}

impl<T: fmt::Debug> MessageBuffer<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: VecDeque::new(),
            max_size,
        }
    }

    #[must_use = "a rejected message is lost unless the caller handles it"]
    pub fn buffer(&mut self, msg: T) -> Result<(), BufferFull<T>> {
        if self.messages.len() >= self.max_size {
            return Err(BufferFull(msg));
        }

        info!("Buffering message: {msg:?}");
        self.messages.push_back(msg);

        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.messages.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_messages_up_to_capacity() {
        let mut buffer = MessageBuffer::new(2);

        assert!(buffer.buffer(1).is_ok());
        assert!(buffer.buffer(2).is_ok());
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn returns_the_message_once_capacity_is_reached() {
        let mut buffer = MessageBuffer::new(1);

        assert!(buffer.buffer(1).is_ok());

        let Err(BufferFull(rejected)) = buffer.buffer(2) else {
            panic!("expected the message to be rejected");
        };

        assert_eq!(rejected, 2);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn a_zero_capacity_buffer_rejects_every_message() {
        let mut buffer = MessageBuffer::new(0);

        assert!(buffer.buffer(1).is_err());
        assert!(buffer.is_empty());
    }

    #[test]
    fn pops_messages_in_insertion_order() {
        let mut buffer = MessageBuffer::new(3);

        for msg in 1..=3 {
            buffer.buffer(msg).unwrap();
        }

        assert_eq!(buffer.pop(), Some(1));
        assert_eq!(buffer.pop(), Some(2));
        assert_eq!(buffer.pop(), Some(3));

        assert!(buffer.is_empty());
        assert_eq!(buffer.pop(), None);
    }
}

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FibonacciBackoff {
    current: u64,
    next: u64,
}

impl FibonacciBackoff {
    pub fn new() -> Self {
        // Start from 1 second
        Self {
            current: 1000,
            next: 1000,
        }
    }
}

impl Iterator for FibonacciBackoff {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        let new_next = self.current + self.next;
        self.current = self.next;
        self.next = new_next;

        Some(Duration::from_millis(self.current))
    }
}

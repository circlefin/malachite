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

#[derive(Debug, Copy, Clone)]
pub struct RangeInclusive {
    pub min: usize,
    pub max: usize,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RangeCmp {
    Less,
    Inclusive,
    Greater,
}

impl RangeInclusive {
    pub fn new(min: usize, max: usize) -> Self {
        Self { min, max }
    }

    pub fn cmp(&self, val: usize) -> RangeCmp {
        if val < self.min {
            RangeCmp::Less
        } else if val > self.max {
            RangeCmp::Greater
        } else {
            RangeCmp::Inclusive
        }
    }
}

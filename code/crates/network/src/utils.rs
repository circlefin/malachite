use std::collections::HashMap;
use std::hash::Hash;

pub(crate) type Slot = usize;

/// Manages the assignment of stable slots (0..N) to entries.
///
/// Ensures O(1) allocation, deallocation, and lookup.
#[derive(Debug)]
pub(crate) struct Slots<T> {
    assigned: HashMap<T, Slot>,
    free: Vec<Slot>,
}

impl<T> Slots<T>
where
    T: Hash + Eq,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            assigned: HashMap::with_capacity(capacity),
            // Initialize in reverse so we pop 0 first
            free: (0..capacity).rev().collect(),
        }
    }

    /// Returns the slot for a entry if it exists.
    pub fn get(&self, entry: &T) -> Option<Slot> {
        self.assigned.get(entry).copied()
    }

    /// Checks if a entry has an assigned slot.
    pub fn contains(&self, entry: &T) -> bool {
        self.get(entry).is_some()
    }

    /// Assigns a slot to a entry.
    /// Returns:
    /// - Some(slot): The newly assigned slot, or the existing slot if already present.
    /// - None: If the allocator is full.
    pub fn assign(&mut self, entry: T) -> Option<Slot> {
        // If already assigned, return existing slot
        if let Some(&slot) = self.assigned.get(&entry) {
            return Some(slot);
        }

        // Try to pop a free slot
        // If none are available, return None
        let slot = self.free.pop()?;
        self.assigned.insert(entry, slot);
        Some(slot)
    }

    /// Frees the slot for a entry.
    /// Returns the freed slot number if the entry was present.
    pub fn release(&mut self, entry: &T) -> Option<Slot> {
        if let Some(slot) = self.assigned.remove(entry) {
            self.free.push(slot);
            return Some(slot);
        }
        None
    }

    /// Returns the number of assigned slots.
    pub fn assigned(&self) -> usize {
        self.assigned.len()
    }

    /// Returns the number of available slots.
    pub fn available(&self) -> usize {
        self.free.len()
    }
}

/// Parsed information from a peer's agent_version string
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pub moniker: String,
    pub address: String,
}

/// Parse agent_version string to extract moniker and consensus address.
///
/// Expected format: "moniker=<name>,address=<addr>" or "moniker=<name>"
/// The order of fields doesn't matter.
///
/// Returns `AgentInfo` with parsed values. Missing fields default to "unknown".
pub fn parse_agent_version(agent_version: &str) -> AgentInfo {
    let mut moniker = String::from("unknown");
    let mut address = String::from("unknown");

    for part in agent_version.split(',') {
        let part = part.trim();
        if let Some(mon) = part.strip_prefix("moniker=") {
            moniker = mon.to_string();
        } else if let Some(addr) = part.strip_prefix("address=") {
            address = addr.to_string();
        }
    }

    AgentInfo { moniker, address }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let slots: Slots<i32> = Slots::new(5);
        assert_eq!(slots.assigned(), 0);
        assert_eq!(slots.available(), 5);
    }

    #[test]
    fn test_sequential_assignment() {
        let mut slots = Slots::new(3);

        // Should assign 0, then 1, then 2
        assert_eq!(slots.assign(10), Some(0));
        assert_eq!(slots.assign(20), Some(1));
        assert_eq!(slots.assign(30), Some(2));

        assert_eq!(slots.assigned(), 3);
        assert_eq!(slots.available(), 0);
    }

    #[test]
    fn test_capacity_limit() {
        let mut slots = Slots::new(2);

        // Fill capacity
        assert_eq!(slots.assign("A"), Some(0));
        assert_eq!(slots.assign("B"), Some(1));

        // Attempt overflow
        assert_eq!(slots.assign("C"), None, "Should return None when full");

        // Verify state hasn't changed
        assert_eq!(slots.assigned(), 2);
        assert!(!slots.contains(&"C"));
    }

    #[test]
    fn test_idempotent_assignment() {
        let mut slots = Slots::new(5);

        // Assign A
        let slot_a = slots.assign('A').unwrap();
        assert_eq!(slot_a, 0);
        assert_eq!(slots.assigned(), 1);

        // Assign A again
        let slot_a_again = slots.assign('A').unwrap();

        // Should be the same slot, and count should not increase
        assert_eq!(slot_a, slot_a_again);
        assert_eq!(
            slots.assigned(),
            1,
            "Assigned count should not increase on re-assignment"
        );
        assert_eq!(slots.available(), 4);
    }

    #[test]
    fn test_lookup_methods() {
        let mut slots = Slots::new(5);
        slots.assign(100);

        // Test get
        assert_eq!(slots.get(&100), Some(0));
        assert_eq!(slots.get(&999), None);

        // Test contains
        assert!(slots.contains(&100));
        assert!(!slots.contains(&999));
    }

    #[test]
    fn test_release() {
        let mut slots = Slots::new(5);
        slots.assign(10);
        assert_eq!(slots.assigned(), 1);

        // Release existing
        let freed_slot = slots.release(&10);
        assert_eq!(freed_slot, Some(0));
        assert_eq!(slots.assigned(), 0);
        assert_eq!(slots.available(), 5);
        assert!(!slots.contains(&10));

        // Release non-existent
        assert_eq!(slots.release(&999), None);
    }

    #[test]
    fn test_recycling_lifo_behavior() {
        let mut slots = Slots::new(3);

        slots.assign("A"); // Slot 0
        slots.assign("B"); // Slot 1
        slots.assign("C"); // Slot 2

        // Release B (slot 1)
        slots.release(&"B");

        // Release A (slot 0)
        slots.release(&"A");

        // Now both 0 and 1 are free.
        // Since we pushed 1 then 0 back onto the stack, 0 is at the top.

        // Next assignment should get slot 0
        assert_eq!(slots.assign("D"), Some(0));

        // Next assignment should get slot 1
        assert_eq!(slots.assign("E"), Some(1));
    }

    #[test]
    fn test_zero_capacity() {
        let mut slots: Slots<i32> = Slots::new(0);
        assert_eq!(slots.available(), 0);
        assert_eq!(slots.assign(1), None);
    }

    #[test]
    fn test_complex_lifecycle_scenario() {
        let mut slots = Slots::new(3);

        // Fill partial
        slots.assign(10); // 0
        slots.assign(20); // 1

        // Release middle
        slots.release(&10); // Frees 0

        // Assign new
        assert_eq!(slots.assign(30), Some(0)); // Should recycle 0

        // Fill remainder
        assert_eq!(slots.assign(40), Some(2)); // Fresh slot

        // Overflow
        assert_eq!(slots.assign(50), None);

        // Release arbitrary
        slots.release(&30); // Frees 0

        // Re-assign overflow candidate
        assert_eq!(slots.assign(50), Some(0));

        // Ensure 20 (slot 1) is still safe
        assert_eq!(slots.get(&20), Some(1));
    }

    #[test]
    fn test_parse_agent_version() {
        let test_cases = [
            // (input, expected_moniker, expected_address)
            ("moniker=node1,address=abc123", "node1", "abc123"),
            ("address=abc123,moniker=node1", "node1", "abc123"), // reversed order
            ("moniker=node1", "node1", "unknown"),
            ("address=abc123", "unknown", "abc123"),
            ("", "unknown", "unknown"),
            (" moniker=node1 , address=abc123 ", "node1", "abc123"), // with whitespace
            ("invalid_format", "unknown", "unknown"),
        ];

        for (input, expected_moniker, expected_address) in test_cases {
            let result = parse_agent_version(input);
            assert_eq!(
                result.moniker, expected_moniker,
                "Failed for input: {:?}",
                input
            );
            assert_eq!(
                result.address, expected_address,
                "Failed for input: {:?}",
                input
            );
        }
    }
}

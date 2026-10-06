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
    #[cfg(test)]
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

/// Maximum number of input Unicode scalar values `sanitize_moniker` considers.
/// Disallowed characters in that window are dropped, so the output can be shorter.
pub(crate) const MAX_MONIKER_INPUT_CHARS: usize = 128;

/// Parsed information from a peer's agent_version string
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pub moniker: String,
}

/// Sanitize a peer-supplied moniker for safe use as a Prometheus label value.
///
/// Keeps ASCII alphanumeric characters plus `-`, `_`, and `.`. Drops everything
/// else. Considers at most [`MAX_MONIKER_INPUT_CHARS`] Unicode scalar values.
/// Empty results become `"unknown"`.
pub(crate) fn sanitize_moniker(moniker: &str) -> String {
    let sanitized: String = moniker
        .chars()
        .take(MAX_MONIKER_INPUT_CHARS)
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        .collect();

    if sanitized.is_empty() {
        String::from("unknown")
    } else {
        sanitized
    }
}

/// Parse agent_version string to extract moniker.
///
/// Expected format: "moniker=<name>"
///
/// Returns `AgentInfo` with a sanitized moniker. Defaults to "unknown" if not found.
pub fn parse_agent_version(agent_version: &str) -> AgentInfo {
    let mut moniker = String::from("unknown");

    for part in agent_version.split(',') {
        let part = part.trim();
        if let Some(mon) = part.strip_prefix("moniker=") {
            moniker = sanitize_moniker(mon);
        }
    }

    AgentInfo { moniker }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_version_defaults_to_unknown() {
        assert_eq!(parse_agent_version("").moniker, "unknown");
        assert_eq!(parse_agent_version("version=1.0").moniker, "unknown");
        assert_eq!(parse_agent_version("moniker=").moniker, "unknown");
        assert_eq!(parse_agent_version("moniker=   ").moniker, "unknown");
    }

    #[test]
    fn parse_agent_version_happy_path() {
        assert_eq!(parse_agent_version("moniker=node-1").moniker, "node-1");
    }

    #[test]
    fn parse_agent_version_drops_disallowed_characters() {
        let info = parse_agent_version("moniker=evil\"}\nnode_safety_failure 0");
        assert_eq!(info.moniker, "evilnode_safety_failure0");
        assert!(!info.moniker.contains('"'));
        assert!(!info.moniker.contains('\n'));
        assert!(!info.moniker.contains(' '));
    }

    #[test]
    fn parse_agent_version_keeps_ascii_identifier_chars() {
        assert_eq!(
            parse_agent_version("moniker=node-1_foo.bar").moniker,
            "node-1_foo.bar"
        );
    }

    #[test]
    fn parse_agent_version_drops_other_controls() {
        let info = parse_agent_version("moniker=a\rb\tc");
        assert_eq!(info.moniker, "abc");
    }

    #[test]
    fn parse_agent_version_truncates_long_moniker() {
        let long = "a".repeat(MAX_MONIKER_INPUT_CHARS + 50);
        let info = parse_agent_version(&format!("moniker={long}"));
        assert_eq!(info.moniker.chars().count(), MAX_MONIKER_INPUT_CHARS);
    }

    #[test]
    fn sanitize_moniker_all_disallowed_becomes_unknown() {
        assert_eq!(sanitize_moniker(r#"\"} 日本語"#), "unknown");
    }

    #[test]
    fn parse_agent_version_last_moniker_wins() {
        assert_eq!(
            parse_agent_version("moniker=first,moniker=second").moniker,
            "second"
        );
    }

    #[test]
    fn sanitize_moniker_drops_backslash() {
        assert_eq!(sanitize_moniker(r"a\b"), "ab");
    }

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
}

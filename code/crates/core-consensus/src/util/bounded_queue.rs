use std::collections::BTreeMap;

/// A data structure that maintains a queue of values associated with monotonically increasing indices.
///
/// # Type Parameters
/// - `I`: The type of the index associated with each value in the queue.
/// - `T`: The type of values stored in the queue.
#[derive(Clone, Debug)]
pub struct BoundedQueue<I, T> {
    capacity: usize,
    queue: BTreeMap<I, Vec<T>>,
}

impl<I, T> BoundedQueue<I, T>
where
    I: Ord,
{
    /// Creates a new `BoundedQueue` with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            queue: BTreeMap::new(),
        }
    }

    /// Push a value into the queue associated with the given index.
    pub fn push(&mut self, index: I, value: T) -> bool
    where
        I: Clone + Ord,
    {
        // If the index already exists, append the value to the existing vector.
        if let Some(values) = self.queue.get_mut(&index) {
            values.push(value);
            return true;
        }

        // If the index does not exist, check if we can add a new entry.
        if self.queue.len() < self.capacity {
            self.queue.insert(index, vec![value]);
            return true;
        }

        // If the queue is full, evict the highest index and insert the new value.
        if let Some((max_index, _)) = self.queue.last_key_value() {
            // If the new index is less than the maximum index, we can evict the maximum index.
            if &index < max_index {
                let max_index = max_index.clone();

                // Remove the highest index
                self.queue.remove(&max_index);

                // Insert the new index with its value
                self.queue.insert(index, vec![value]);

                return true;
            }
        }

        false
    }

    /// Combination of `shift` and `take` methods.
    pub fn shift_and_take(&mut self, min_index: &I) -> impl Iterator<Item = T> {
        self.shift(min_index);
        self.take(min_index)
    }

    /// Remove all entries with indices less than `min_index`.
    pub fn shift(&mut self, min_index: &I) {
        self.queue.retain(|index, _| index >= min_index);
    }

    /// Take all entries with indices equal to `index` and return them.
    pub fn take(&mut self, index: &I) -> impl Iterator<Item = T> {
        self.queue
            .remove(index)
            .into_iter()
            .flat_map(|values| values.into_iter())
    }

    /// Whether the queue is full
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_within_capacity() {
        let mut queue = BoundedQueue::new(3);

        assert!(queue.push(1, "value1"));
        assert!(queue.push(2, "value2"));
        assert!(queue.push(3, "value3"));

        assert_eq!(queue.queue.len(), 3);
        assert_eq!(queue.queue.get(&1).unwrap(), &vec!["value1"]);
        assert_eq!(queue.queue.get(&2).unwrap(), &vec!["value2"]);
        assert_eq!(queue.queue.get(&3).unwrap(), &vec!["value3"]);
    }

    #[test]
    fn push_to_existing_index() {
        let mut queue = BoundedQueue::new(2);
        queue.push(10, "a");

        // Push another value to the same index
        let result = queue.push(10, "b");

        assert!(result);
        // The number of unique indices should not change
        assert_eq!(queue.queue.len(), 1);
        // The vector at the index should now contain both values
        assert_eq!(queue.queue.get(&10), Some(&vec!["a", "b"]));
    }

    #[test]
    fn push_to_full_queue_fails_for_new_index() {
        let mut queue = BoundedQueue::new(2);
        queue.push(10, "a");
        queue.push(20, "b");

        // Assert queue is full
        assert_eq!(queue.queue.len(), queue.capacity);

        // Try to push a new index to a full queue
        let result = queue.push(30, "c");

        assert!(!result, "Push should fail when capacity is reached");
        assert_eq!(queue.queue.len(), 2, "Queue length should not change");
        assert!(
            !queue.queue.contains_key(&30),
            "The new element should not be added"
        );
    }

    #[test]
    fn push_to_full_queue_succeeds_for_existing_index() {
        let mut queue = BoundedQueue::new(2);
        queue.push(10, "a");
        queue.push(20, "b");

        // Assert queue is full
        assert_eq!(queue.queue.len(), queue.capacity);

        // Push to an existing index on a full queue
        let result = queue.push(10, "c");

        assert!(result, "Push should succeed for an existing index");
        assert_eq!(
            queue.queue.len(),
            2,
            "Queue length (number of indices) should not change"
        );
        assert_eq!(
            queue.queue.get(&10),
            Some(&vec!["a", "c"]),
            "The new value should be appended"
        );
    }

    #[test]
    fn push_to_zero_capacity_queue() {
        let mut queue = BoundedQueue::new(0);
        let result = queue.push(1, "a");

        assert!(!result);
        assert!(queue.queue.is_empty());
    }

    #[test]
    fn take_existing_index() {
        let mut queue = BoundedQueue::new(3);
        queue.push(10, "a");
        queue.push(10, "b");
        queue.push(20, "c");

        // Take the values for index 10
        let taken_values: Vec<&str> = queue.take(&10).collect();

        assert_eq!(taken_values, vec!["a", "b"]);
        assert_eq!(
            queue.queue.len(),
            1,
            "The entry for index 10 should be removed"
        );
        assert!(
            !queue.queue.contains_key(&10),
            "The key 10 should no longer exist"
        );
        assert!(
            queue.queue.contains_key(&20),
            "The key 20 should still exist"
        );
    }

    #[test]
    fn take_non_existent_index() {
        let mut queue = BoundedQueue::new(3);
        queue.push(10, "a");

        // Take a non-existent index
        let taken_values: Vec<&str> = queue.take(&99).collect();

        assert!(taken_values.is_empty());
        assert_eq!(queue.queue.len(), 1, "Queue should be unchanged");
    }

    #[test]
    fn take_empty_queue() {
        let mut queue: BoundedQueue<i32, String> = BoundedQueue::new(5);
        let values: Vec<_> = queue.take(&1).collect();
        assert!(values.is_empty());
    }

    #[test]
    fn shift_removes_older_entries() {
        let mut queue = BoundedQueue::new(5);
        queue.push(10, 1);
        queue.push(20, 2);
        queue.push(30, 3);
        queue.push(40, 4);

        // Shift to remove all indices less than 30
        queue.shift(&30);

        assert_eq!(queue.queue.len(), 2);
        assert!(!queue.queue.contains_key(&10));
        assert!(!queue.queue.contains_key(&20));
        assert!(queue.queue.contains_key(&30)); // Should not be removed
        assert!(queue.queue.contains_key(&40));
    }

    #[test]
    fn shift_all_and_none() {
        let mut queue = BoundedQueue::new(5);
        queue.push(10, 1);
        queue.push(20, 2);

        // Act: Shift with an index greater than all keys
        queue.shift(&30);
        // Assert: All entries should be removed
        assert!(queue.queue.is_empty());

        // Arrange again
        queue.push(10, 1);
        queue.push(20, 2);

        // Act: Shift with an index smaller than all keys
        queue.shift(&5);
        // Assert: No entries should be removed
        assert_eq!(queue.queue.len(), 2);
        assert!(queue.queue.contains_key(&10));
        assert!(queue.queue.contains_key(&20));
    }

    #[test]
    fn shift_on_empty_queue() {
        let mut queue = BoundedQueue::<u32, i32>::new(5);
        // Shift on an empty queue should not panic
        queue.shift(&100);
        assert!(queue.queue.is_empty());
    }

    #[test]
    fn shift_and_take_removes_older_and_takes_target() {
        let mut queue = BoundedQueue::new(5);
        queue.push(1, "value1");
        queue.push(2, "value2a");
        queue.push(2, "value2b");
        queue.push(3, "value3");
        queue.push(4, "value4");

        // Shift removes keys < 2 (i.e., key 1), then takes values from key 2
        let values: Vec<_> = queue.shift_and_take(&2).collect();

        assert_eq!(values, vec!["value2a", "value2b"]);
        assert!(!queue.queue.contains_key(&1), "Key 1 should be shifted");
        assert!(!queue.queue.contains_key(&2), "Key 2 should be taken");
        assert!(queue.queue.contains_key(&3));
        assert!(queue.queue.contains_key(&4));
        assert_eq!(queue.queue.len(), 2);
    }

    #[test]
    fn shift_and_take_with_non_existent_target() {
        let mut queue = BoundedQueue::new(5);
        queue.push(1, "value1");
        queue.push(3, "value3");
        queue.push(5, "value5");

        // Shift removes keys < 4 (i.e., 1 and 3), then takes from non-existent key 4
        let values: Vec<_> = queue.shift_and_take(&4).collect();

        assert!(
            values.is_empty(),
            "Should return empty for non-existent key"
        );
        assert!(!queue.queue.contains_key(&1), "Key 1 should be shifted");
        assert!(!queue.queue.contains_key(&3), "Key 3 should be shifted");
        assert!(queue.queue.contains_key(&5), "Key 5 should remain");
        assert_eq!(queue.queue.len(), 1);
    }

    #[test]
    fn clone_trait() {
        let mut queue = BoundedQueue::new(3);
        queue.push(1, "value1");
        queue.push(2, "value2");

        let cloned_queue = queue.clone();

        assert_eq!(queue.capacity, cloned_queue.capacity);
        assert_eq!(queue.queue, cloned_queue.queue);
        // Ensure it's a deep clone by modifying one and checking the other
        queue.push(1, "another_value");
        assert_ne!(queue.queue, cloned_queue.queue);
    }

    #[test]
    fn debug_trait() {
        let mut queue = BoundedQueue::new(2);
        queue.push(1, "value1");

        let debug_str = format!("{:?}", queue);
        assert!(debug_str.contains("BoundedQueue"));
        assert!(debug_str.contains("capacity: 2"));
        assert!(debug_str.contains("queue: {1: [\"value1\"]}"));
    }

    #[test]
    fn with_different_types() {
        // Test with different index and value types to ensure generics work
        let mut queue: BoundedQueue<String, i32> = BoundedQueue::new(3);

        assert!(queue.push("key1".to_string(), 100));
        assert!(queue.push("key2".to_string(), 200));
        assert!(queue.push("key1".to_string(), 150));

        let values: Vec<_> = queue.take(&"key1".to_string()).collect();
        assert_eq!(values, vec![100, 150]);
    }

    #[test]
    fn ordering_with_btreemap() {
        let mut queue = BoundedQueue::new(5);

        // Insert in non-sequential order
        queue.push(3, "value3");
        queue.push(1, "value1");
        queue.push(4, "value4");
        queue.push(2, "value2");

        // BTreeMap should maintain key order
        let keys: Vec<_> = queue.queue.keys().cloned().collect();
        assert_eq!(keys, vec![1, 2, 3, 4]);
    }

    #[test]
    fn edge_case_single_capacity() {
        let mut queue = BoundedQueue::new(1);

        assert!(queue.push(1, "value1"));

        assert!(
            queue.push(1, "value1_again"),
            "Should succeed on existing index"
        );

        assert!(
            !queue.push(2, "value2"),
            "Should fail on new index when full"
        );

        let values: Vec<_> = queue.take(&1).collect();
        assert_eq!(values, vec!["value1", "value1_again"]);
        assert!(queue.queue.is_empty());
    }

    #[test]
    fn multiple_operations_sequence() {
        let mut queue = BoundedQueue::new(4);

        // Initial setup
        queue.push(1, "a");
        queue.push(2, "b");
        queue.push(3, "c");
        queue.push(4, "d");
        assert_eq!(queue.queue.len(), 4);

        // Shift and verify
        queue.shift(&3); // Removes keys < 3 (i.e., 1 and 2)
        assert_eq!(queue.queue.len(), 2);
        assert!(!queue.queue.contains_key(&2));
        assert!(queue.queue.contains_key(&3));

        // Take and verify
        let values: Vec<_> = queue.take(&3).collect();
        assert_eq!(values, vec!["c"]);
        assert_eq!(queue.queue.len(), 1);
        assert!(!queue.queue.contains_key(&3));

        // Add more values, filling capacity
        assert!(queue.push(5, "e"));
        assert!(queue.push(6, "f"));
        assert!(queue.push(7, "g"));
        assert_eq!(queue.queue.len(), 4);

        // This should fail - at capacity and 8 is greater than 7
        assert!(!queue.push(8, "h"));

        // Final verification
        let keys: Vec<_> = queue.queue.keys().cloned().collect();
        assert_eq!(keys, vec![4, 5, 6, 7]);
    }

    #[test]
    fn push_out_of_order_to_full_queue() {
        let mut queue = BoundedQueue::new(2);
        queue.push(10, "a");
        queue.push(30, "b");

        // Assert queue is full
        assert_eq!(queue.queue.len(), queue.capacity);

        // Try to push a new index to a full queue
        let result = queue.push(20, "c");

        assert!(result, "Push should succeed");
        assert_eq!(queue.queue.len(), 2, "Queue length should not change");

        assert!(
            queue.queue.contains_key(&20),
            "The new element should be added"
        );

        assert!(
            !queue.queue.contains_key(&30),
            "The high value element should be remmoved"
        );
    }
}

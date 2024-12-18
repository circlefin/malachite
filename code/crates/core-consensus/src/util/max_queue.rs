/// A data structure that maintains a queue of values associated with monotonically increasing indices,
/// retaining only those values associated with the maximum index seen so far.
///
/// # Type Parameters
/// - `I`: The type of the index associated with each value in the queue.
/// - `T`: The type of values stored in the queue.
///
/// # Invariant
/// - All values in the queue are associated with the maximum index observed so far.
#[derive(Clone, Debug)]
pub struct MaxQueue<I, T> {
    /// The highest index observed, which determines the values retained in the queue.
    highest_index: I,

    /// A vector storing the values associated with the maximum index.
    /// Values are appended in the order they are pushed.
    queue: Vec<T>,
}

impl<I, T> Default for MaxQueue<I, T>
where
    I: Default,
{
    /// Creates a `MaxQueue` with the default index value and an empty queue.
    ///
    /// # Returns
    /// - A `MaxQueue` instance with `current` initialized to the default value of `I` and an empty `queue`.
    fn default() -> Self {
        Self {
            highest_index: Default::default(),
            queue: Default::default(),
        }
    }
}

impl<I, T> MaxQueue<I, T> {
    /// Constructs a new, empty `MaxQueue` with its index set to default.
    ///
    /// # Returns
    /// - A new `MaxQueue` with default `current` index and an empty queue.
    pub fn new() -> Self
    where
        I: Default,
    {
        Self::default()
    }

    /// Pushes a value into the queue with an associated index.
    ///
    /// - If the `index` is greater than the highest index seen so far, the queue is cleared,
    ///   the highest index seen so far is updated, and the value is added.
    /// - If the `index` is equal to the highest index seen so far, the value is appended to the queue.
    /// - If the `index` is less than the highest index seen so far, the value is ignored.
    ///
    /// # Arguments
    /// - `index`: The index associated with the value.
    /// - `value`: The value to be stored in the queue.
    ///
    /// # Returns
    /// - Whether or not the value was inserted into the queue.
    #[allow(clippy::comparison_chain)]
    pub fn push(&mut self, index: I, value: T) -> bool
    where
        I: Ord,
    {
        if index > self.highest_index {
            // New highest index, clear the queue, insert the new value
            self.highest_index = index;
            self.queue.clear();
            self.queue.push(value);
            true
        } else if index == self.highest_index {
            // Same index, insert the new value
            self.queue.push(value);
            true
        } else {
            // Smaller index, ignore the value
            false
        }
    }

    /// Returns an iterator over references to the values in the queue.
    ///
    /// # Returns
    /// - An iterator producing references to each value stored in the queue in order of insertion.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.queue.iter()
    }

    /// Returns how many values are stored in queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns the queue.
    pub fn into_vec(self) -> Vec<T> {
        self.queue
    }

    /// Returns a clone of the queue.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.queue.to_vec()
    }
}

/// Consumes the `MaxQueue` and returns an iterator that yields its values.
///
/// # Returns
/// - An iterator over values in the queue.
impl<I, T> IntoIterator for MaxQueue<I, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.queue.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_queue() {
        let mut queue = MaxQueue::new();

        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        assert!(queue.push(1, "one"));
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        assert_eq!(queue.to_vec(), vec!["one"]);

        assert!(queue.push(2, "two"));
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        assert_eq!(queue.to_vec(), vec!["two"]);

        assert!(!queue.push(1, "one again"));
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        assert_eq!(queue.to_vec(), vec!["two"]);

        assert!(queue.push(2, "two again"));
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());
        assert_eq!(queue.to_vec(), vec!["two", "two again"]);

        assert!(queue.push(3, "three"));
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        assert_eq!(queue.to_vec(), vec!["three"]);
    }
}

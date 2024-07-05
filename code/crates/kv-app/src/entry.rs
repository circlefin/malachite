/// Very basic data structure representing an entry in a KV store

use std::str::FromStr;

#[allow(dead_code)]
pub struct Entry {
    pub key: String,
    pub value: String,
}

impl Entry {
    fn new(p0: &str, p1: &str) -> Entry {
        Entry {
            key: p0.into(),
            value: p1.into(),
        }
    }
}

impl FromStr for Entry {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(' ').collect();
        // There should be two parts
        if parts.len() != 2 {
            Err(())
        } else {
            Ok(Entry::new(parts[0], parts[1]))
        }
    }
}
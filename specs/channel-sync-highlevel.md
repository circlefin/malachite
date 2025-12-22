# Channel sync – high-level notes

This document captures a high-level view of how channel-based replication and sync are expected to behave in Malachite.

It is not a formal specification. Instead, it is a starting point for discussions and more detailed specs.

## Goals

- Keep replicas in sync with minimal latency under normal network conditions.
- Provide clear semantics for how messages are ordered and committed.
- Make it easy to reason about the behavior of applications built on top of channels.

## Questions to answer

When refining this document into a full spec, we should explicitly answer:

- How do channels behave under sustained network partitions?
- What are the guarantees about message delivery and reordering?
- How do we expose progress (e.g. "all messages up to index N are committed") to applications?

As these questions are clarified and implemented, this document can be updated with links to more detailed protocol descriptions and test plans.

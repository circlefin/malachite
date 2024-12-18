# Analysis of the ValueSync Specification

This document contains a report on analysis of ValueSync. We have two versions of the
state machine, one with consensus abstracted away, and one in which the ValueSync
state machine and the consensus state machine are combined.

## Invariants checked with quint run 

- `validRequestInvariant`: A request should only be sent to a server who has reported, via status message, having data for the requested height.
- `noOldRequestsInv`: A client doesn't have open requests for past heights
- `serverRespondsToRequestingPeersInvariant`: A server only replies to a request received from a client (The client request might have timed out).

## Witnesses

- `serverRespondsToRequestingPeersWitness`: This witness should report a scenario where a request timeouts, the client submits a new one, and a late response is received.

## Temporal properties

We don't check these properties but record them for documentation purposes.
- `terminationRequest`: Every request will eventually terminate. 



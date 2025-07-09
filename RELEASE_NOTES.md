# Release Notes

## Unreleased

- Rename `Effect::RebroadcastVote` to `Effect::RepublishVote` ([#1011](https://github.com/informalsystems/malachite/issues/1011))

## 0.4.0

*July 8th, 2025*

- Add parallel requests for the sync module ([#1092](https://github.com/informalsystems/malachite/issues/1092))

## 0.3.1

*July 7th, 2025*

- Derive [Borsh](https://borsh.io) encoding for all core types, behind a `borsh` feature flag ([#1098](https://github.com/informalsystems/malachite/pull/1098))
- Fixed a bug where the consensus engine would panic when the validator set is empty, now an error is properly emitted in the logs ([#1111](https://github.com/informalsystems/malachite/pull/1111))
- When the sync module receives an invalid commit certificate from another peer, it will now drop the associated synced value altogether instead of passing it up to the application ([#1112](https://github.com/informalsystems/malachite/pull/1112))

## 0.3.0

*June 17th, 2025*

- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness ([#998](https://github.com/informalsystems/malachite/issues/998))
- Reply to `GetValidatorSet` is now optional ([#990](https://github.com/informalsystems/malachite/issues/990))
- Clarify and improve the application handling of multiple proposals for same height and round ([#833](https://github.com/informalsystems/malachite/issues/833))
- Prune votes and polka certificates that are from lower rounds than node's `locked_round` ([#1019](https://github.com/informalsystems/malachite/issues/1019))
- Add support for making progress in the presence of equivocating proposals ([#1018](https://github.com/informalsystems/malachite/issues/1018))
- Take minimum available height into account when requesting values from peers ([#1074](https://github.com/informalsystems/malachite/issues/1074))
- Add peer scoring system to the sync module with customizable scoring strategy ([#1072](https://github.com/informalsystems/malachite/issues/1072))
  [See the corresponding PR](https://github.com/informalsystems/malachite/pull/1071) for more details.

## 0.2.0

*April 16th, 2025*

- Add the capability to re-run consensus for a given height ([#893](https://github.com/informalsystems/malachite/issues/893))
- Verify polka certificates ([#974](https://github.com/informalsystems/malachite/issues/974))
- Use aggregated signatures in polka certificates ([#915](https://github.com/informalsystems/malachite/issues/915))
- Improve verification of commit certificates ([#974](https://github.com/informalsystems/malachite/issues/974))

## 0.1.0

*April 9th, 2025*

This is the first release of the Malachite consensus engine intended for general use.
This version introduces production-ready functionality with improved performance and reliability.

### Changes

See the full list of changes in the [CHANGELOG](CHANGELOG.md#0.1.0).

### Resources

- [The tutorial][tutorial] for building a simple application on top of Malachite using the high-level channel-based API.
- [ADR 003][adr-003] describes the architecture adopted in Malachite for handling the propagation of proposed values.
- [ADR 004][adr-004] describes the coroutine effect system used in Malachite.
  It is relevant if you are interested in building your own engine on top of the core consensus implementation of Malachite.


[tutorial]: ./docs/tutorials/channels.md
[adr-003]: ./docs/architecture/adr-003-values-propagation.md
[adr-004]: ./docs/architecture/adr-004-coroutine-effect-system.md

## 0.0.1

*December 19, 2024*

First open-source release of Malachite.
This initial version provides the foundational consensus implementation but is not recommended for production use.

### Changes

See the full list of changes in the [CHANGELOG](CHANGELOG.md#0.0.1).

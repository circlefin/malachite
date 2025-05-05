# Release Notes

## Unreleased

- Reply to `GetValidatorSet` is now optional ([#990](https://github.com/informalsystems/malachite/issues/990))

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

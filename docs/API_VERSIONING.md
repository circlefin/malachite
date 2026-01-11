# API Versioning Strategy

This document describes how Malachite services should expose and evolve their APIs.

## Version identifiers

- Use URL prefixing (e.g. `/v1/`, `/v2/`) to separate incompatible API versions.
- Increment the version number only when breaking changes occur.
- Minor, backwards-compatible changes should not bump the major version.

## Deprecation policy

- When introducing a new major version, mark the old version as deprecated.
- Provide a sunset timeline (e.g. 6 months) and communicate via changelog.
- Maintain backwards compatibility during the deprecation period.

## Documentation

- Each API version must have its own set of documentation.
- Include migration guides when removing or altering endpoints.

Consistent versioning allows clients to upgrade smoothly and reduces integration friction.

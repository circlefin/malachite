# Node operator readiness checklist

This document provides a practical checklist for operators who plan to
run Malachite as a consensus engine in production or pre-production
environments.

It complements the architecture and overview already described in the
README and ARCHITECTURE documents by focusing on operational details.

---

## 1. Repository and version

- Confirm that you are using a released version of Malachite, or a
  specific commit that you are comfortable running.
- Review the release notes for any breaking changes that might affect
  your deployment.
- If possible, test new releases in a staging environment before
  upgrading production nodes.

---

## 2. Rust and toolchain

Malachite requires a recent Rust toolchain.

Before building:

- check your active Rust version with:

  ```bash
  rustc --version

# Architecture diagram notes

This file documents the purpose of the diagrams stored under `assets/` and how they relate to the Malachite codebase.

## Intended usage

- Architecture diagrams can be embedded in Markdown specs under `specs/` or in external documentation.
- When updating a diagram, ensure that:
  - The file name is stable (to avoid breaking links).
  - The corresponding spec or doc is updated in the same pull request.

## Suggested conventions

- Prefer vector formats (e.g. SVG) when possible.
- Keep text labels short and use terminology consistent with the rest of the project.
- When adding a new diagram, include a short caption in the relevant spec that explains what it represents.

## Keeping diagrams in sync

When you make a protocol or API change that affects a diagram:

1. Update the diagram source.
2. Export the updated asset into this directory.
3. Update any specs that embed or reference the diagram.
4. Mention the updated diagrams explicitly in your pull request description.

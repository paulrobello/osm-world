# Contributing to osm-world

Guide for setting up a development environment, running checks, and submitting changes.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Running Tests](#running-tests)
- [Running Checks](#running-checks)
- [Commit Messages](#commit-messages)
- [Branch Naming](#branch-naming)
- [Pull Request Process](#pull-request-process)

## Prerequisites

- Rust 1.92 (the `rust-version` declared in `Cargo.toml`)
- Bun 1.3.x (for the Web Explorer frontend)
- A GPU with WGPU-compatible drivers

The `par-osm-rust` data-source crate is vendored in-tree at `crates/par-osm-rust` and builds as part of the workspace, so no sibling checkout is required.

## Development Setup

```bash
git clone https://github.com/paulrobello/osm-world
cd osm-world
make build
make web-install
```

Run the renderer with the built-in test scene:

```bash
make run
```

Start both the Rust API server and the Web Explorer:

```bash
make dev
```

## Code Style

### Rust

- Run `make fmt` before committing. This runs `cargo fmt --check`.
- Run `make lint` before committing. This runs `cargo clippy`.
- Follow the existing patterns for error handling: use `anyhow` for application errors and structured enums for API errors.
- Add `///` doc comments on public functions, structs, and enums.
- Add `//!` module-level doc to each module file.
- Keep tests co-located with the code they test in `#[cfg(test)] mod tests` blocks.
- Avoid `unwrap()` in production code. Use `?`, `map_err`, or explicit error variants.

### TypeScript / Web

- Use TypeScript for all new files in `web/src/`.
- Run `bun run build` in `web/` to verify the build passes.
- Run `bun test` in `web/` to run frontend tests.
- Follow the existing export patterns in `web/src/lib/`.

### Shaders (WGSL)

- Add top-of-file comments explaining the shader purpose.
- Keep uniform layouts documented.

## Running Tests

```bash
make test
```

For web-only tests:

```bash
cd web
bun test
```

## Running Checks

Run the full check suite before opening a pull request:

```bash
make checkall
```

This runs formatting, type checking, linting, and tests. All four must pass.

Individual targets:

| Target | Command |
| --- | --- |
| Build | `make build` |
| Format check | `make fmt` |
| Type check | `make typecheck` |
| Lint | `make lint` |
| Test | `make test` |
| Web build | `make web-build` |
| Clean | `make clean` |

## Commit Messages

Write commit messages in imperative mood: "Add feature" rather than "Added feature" or "Adds feature".

Keep the first line under 72 characters. Add a blank line and a longer description if the change needs context.

Example:

```text
Fix polygon winding for water features

Water polygons with clockwise winding were invisible in the renderer.
Normalize all closed polygons to counter-clockwise before mesh generation.
```

## Branch Naming

Use descriptive branch names with a short prefix:

- `feat/short-description` for new features
- `fix/short-description` for bug fixes
- `docs/short-description` for documentation changes
- `refactor/short-description` for code reorganization

## Pull Request Process

1. Create a branch from `main`.
2. Make focused changes. Avoid mixing features, refactors, and documentation in one PR.
3. Run `make checkall` and fix any failures.
4. Open a pull request against `main`.
5. Describe the change, the motivation, and how it was tested.
6. Respond to review feedback. Request re-review after addressing comments.

### PR Checklist

- [ ] `make checkall` passes locally
- [ ] New public functions have `///` doc comments
- [ ] New modules have `//!` module-level doc
- [ ] Tests cover the new behavior or the fix
- [ ] No unrelated formatting changes mixed in

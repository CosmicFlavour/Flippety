# Contributing to Flippety

Thanks for considering a contribution. This is a small, personal project, so the process is deliberately lightweight — but a few conventions keep the codebase consistent.

## Getting set up

See the [README](README.md#getting-started) for prerequisites and how to run the app. In short:

```bash
pnpm install
pnpm tauri dev
```

## Before you open a PR

1. **Discuss non-trivial changes first.** For anything bigger than a bug fix (new features, schema changes, UI redesigns), open an issue describing what you want to do before writing code. It avoids wasted work if the direction doesn't fit the project.
2. **Keep PRs focused.** One logical change per PR is much easier to review than a bundle of unrelated fixes.
3. **Add tests for behavior changes.** Both the Rust backend and the React frontend have real test suites — see [Testing](#testing) below. A PR that changes behavior without a test covering it will likely get asked for one.
4. **Run the checks locally before pushing** (see [Pre-commit hooks](#pre-commit-hooks)) so CI/review isn't spent catching formatting or lint issues.

## Code conventions

### Backend (Rust)

- Every `#[tauri::command]` should be a thin wrapper around a plain, testable `..._inner(conn: &Connection, ...)` function. This lets tests exercise real logic against a real (in-memory) SQLite connection without spinning up Tauri — see any file under `src-tauri/src/commands/` for the pattern.
- Validation and business logic belong in Rust, not the frontend. The frontend is intentionally a thin, typed passthrough (`src/lib/api.ts`) — don't duplicate validation there.
- Reuse before you add. If you're about to write a loop that seeds/prunes review state, parses a `Direction`, or otherwise repeats something that already exists in `src-tauri/src/db/`, extract or reuse instead of copy-pasting.
- New SQL schema changes go in a new numbered migration file under `src-tauri/src/db/migrations/`, appended to the `MIGRATIONS` array in `db/mod.rs`. Never edit an existing migration that's already shipped.
- Run `cargo fmt` and `cargo clippy -- -D warnings` before committing — clippy warnings are treated as errors in this repo.

### Frontend (TypeScript / React)

- Match the existing component structure: form dialogs (`*FormDialog.tsx`) own their own local state and call a passed-in `onSubmit`/`onDelete`/etc.; pages (`src/pages/`) own data fetching (TanStack Query) and wire mutations to those components.
- Types in `src/types/models.ts` should mirror the Rust models in `src-tauri/src/models/` field-for-field — if you change one, change the other.
- Run `pnpm exec tsc --noEmit` — the project has no `any`-typed escape hatches to lean on.

## Testing

```bash
# Backend
cd src-tauri && cargo test

# Frontend
pnpm test
```

Rust tests live inline in `#[cfg(test)] mod tests` blocks next to the code they cover. Frontend tests use Vitest + Testing Library and live alongside the component as `*.test.tsx`. Favor testing behavior (what a user/caller observes) over implementation details.

## Pre-commit hooks

The repo ships a `.pre-commit-config.yaml` that runs `cargo fmt`, `cargo clippy`, `tsc --noEmit`, and `vitest run` on commit. To enable it locally:

```bash
pip install pre-commit
pre-commit install
```

## Commit messages

Commits in this repo follow a loose [Conventional Commits](https://www.conventionalcommits.org/) style: `type(scope): summary`, e.g. `fix(backend): wrap import in a transaction` or `feat(frontend): add reset-progress action`. Common types: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`. Not strictly enforced, but appreciated.

## Questions

Open an issue — happy to discuss ideas or point you at the right part of the codebase.

# Flippety

A local-first, cross-platform flashcard app built on [Tauri](https://tauri.app/), with spaced repetition powered by [FSRS](https://github.com/open-spaced-repetition/rs-fsrs). No account, no server, no telemetry — your decks live in a SQLite database on your own machine.

The card schema is domain-agnostic: any subject that fits a "prompt one side, reveal the full answer, rate your own recall" loop works (languages, formulas, definitions, anything).

## Features

- **Bidirectional cards.** Each card can drill both directions (e.g. `dog → 狗` and `狗 → dog`) as independently-scheduled review items, since recognition and production are different skills.
- **FSRS scheduling.** Review intervals adapt to how well you actually know each card, using the [`rs-fsrs`](https://crates.io/crates/rs-fsrs) implementation with default retention parameters.
- **Self-graded review.** The app never auto-checks your answer — it shows a prompt, you think it through, reveal the full card, and rate your own recall (Again / Hard / Good / Easy).
- **Leveled new-card introduction.** Give cards a level (curated decks can order e.g. HSK1 before HSK2) and cap how many new cards a deck introduces per day. New cards are fully shuffled within a level rather than blocked by topic, favoring the better-for-retention "interleaving" effect over the easier-feeling but worse-retained "blocked practice" one.
- **Import / export.** Decks are portable as a single JSON file — content only, no review history — so you can back up, share, or re-import a deck without ever duplicating or resetting your progress. Re-importing a deck you already have merges by card ID.
- **Local-first.** Everything lives in a SQLite file in your platform's app data directory. No network calls, no account.
- **Light / dark / system theme.**

## Tech stack

- **Backend:** Rust, [Tauri v2](https://tauri.app/), [`rusqlite`](https://crates.io/crates/rusqlite) (bundled SQLite), [`rs-fsrs`](https://crates.io/crates/rs-fsrs)
- **Frontend:** React 19, TypeScript, Vite, Tailwind CSS, [TanStack Query](https://tanstack.com/query), shadcn-style components on [base-ui](https://base-ui.com/)

## Getting started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Node.js](https://nodejs.org/) and [pnpm](https://pnpm.io/installation)
- Platform build tools for Tauri — see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/) for your OS

### Setup

```bash
pnpm install
```

### Run in development

```bash
pnpm tauri dev
```

This opens a native window backed by a local Vite dev server, with hot reload on the frontend and a rebuild-on-change loop on the Rust backend.

### Build a release bundle

```bash
pnpm tauri build
```

Produces a platform-native installer/bundle in `src-tauri/target/release/bundle/`.

### Tests

```bash
# Frontend (Vitest)
pnpm test

# Backend (Rust)
cd src-tauri && cargo test
```

## Project layout

```
src/                  React frontend
  components/          Reusable UI (forms, dialogs, shadcn-style primitives)
  pages/                Top-level views (decks, cards, study)
  lib/                  Typed API layer over Tauri commands, theme, utils
  types/                TypeScript mirrors of the Rust models

src-tauri/            Rust backend
  src/commands/         Tauri commands (thin wrappers around testable `_inner` fns)
  src/db/                SQLite access layer + migrations
  src/models/            Shared data structures
  src/srs/                FSRS scheduling glue
```

The frontend holds no business logic — validation, scheduling, and persistence all happen in Rust, and the frontend is a thin, typed passthrough to Tauri commands (see `src/lib/api.ts`).

## Data storage

Flippety stores its SQLite database in the platform-appropriate app data directory (via Tauri's `app_data_dir()`), e.g. `~/.local/share/io.github.cosmicflavour.flippety/flippety.db` on Linux. Deleting that file resets the app to a blank state.

## Deck JSON format

Import/export uses a single JSON file per deck. It's content only — no FSRS scheduling state, no review history — so importing or re-importing a deck never resets progress on cards it already knows about (see [Data storage](#data-storage) for where that progress actually lives).

```json
{
  "deck": {
    "name": "Chinese HSK1",
    "description": "Core HSK1 vocabulary"
  },
  "cards": [
    {
      "id": "3a1f9e2c-2b7a-4e3a-9f1a-6b8b6a2b9d10",
      "face_1": "dog",
      "face_2": "狗",
      "full": {
        "title": "狗",
        "subtitle": "gǒu",
        "body": "Domestic dog. Composed of 犭(dog radical) + 句(sound).",
        "foot": "这是我的狗。 — This is my dog."
      },
      "tags": ["animals", "hsk1"],
      "directions": ["1->2", "2->1"],
      "level": 1
    }
  ]
}
```

| Field                | Type                   | Required | Notes                                                                                                                                                                                          |
| -------------------- | ---------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `deck.name`          | string                 | yes      | Trimmed and validated non-empty on import.                                                                                                                                                     |
| `deck.description`   | string or `null`       | no       |                                                                                                                                                                                                |
| `cards[].id`         | string                 | no       | Omit for a new card. On **merge** import, a card whose `id` matches an existing card updates its content in place without touching its review progress; a new/omitted `id` creates a new card. |
| `cards[].face_1`     | string                 | yes      | The face_1 → face_2 prompt. Must be non-empty.                                                                                                                                                 |
| `cards[].face_2`     | string                 | yes      | The face_2 → face_1 prompt. Must be non-empty.                                                                                                                                                 |
| `cards[].full`       | object                 | yes      | The revealed answer: `{ "title", "subtitle", "body", "foot" }`, all strings (empty string is fine, the field just can't be omitted).                                                           |
| `cards[].tags`       | string[]               | no       | Defaults to `[]`.                                                                                                                                                                              |
| `cards[].directions` | `("1->2" \| "2->1")[]` | no       | Which direction(s) generate review items. Defaults to both. Must contain at least one entry.                                                                                                   |
| `cards[].level`      | integer                | no       | Introduction order — lower levels are introduced to you before higher ones. Defaults to `1`.                                                                                                   |

Export always includes every field (including `id`, so re-importing round-trips card identity); import fills in the optional ones with the defaults above when they're missing, so hand-written or older-format decks still work.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

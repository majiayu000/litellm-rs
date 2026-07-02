# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@5ef936c47c57`, 33 Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/storage/database/seaorm_db/team_repository.rs` at 932 lines.
Unlike the prior cost type tranche, this is runtime repository code, so the split must follow
storage and synchronization responsibilities instead of only extracting tests.

## Architecture Principles

1. Facade compatibility: when splitting public type files, the original module path keeps
   re-exporting the same public names with `pub use`.
2. Runtime ownership: runtime files split by one responsibility at a time, such as request
   mapping, response mapping, operation handlers, storage helpers, or error conversion.
3. Test-suite ownership: test-only files split by behavior domain while keeping original
   assertions and focused test command coverage.
4. No silent degradation: moved code must preserve current error propagation and must not
   add warning-only fallbacks for previously failing states.
5. Bounded PRs: each tranche owns one file family and includes line-count proof plus focused
   tests.

## Queue Design

| Phase | Lane | Target examples | Verification pattern |
| --- | --- | --- | --- |
| P1 | Test suites | router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | SeaORM team repository, OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: SeaORM Team Repository Responsibility Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Root repository module | `src/storage/database/seaorm_db/team_repository.rs` | Defines `SeaOrmTeamRepository`, helpers, and the full `TeamRepository` impl. | The file is 932 lines and mixes four responsibilities. |
| Canonical storage | `teams`, `team_members` helpers | Stores canonical `Team` and `TeamMember` JSON snapshots with backend-specific placeholders. | Should be isolated from legacy synchronization and trait orchestration. |
| Legacy sync | `um_teams` and legacy users | Backfills legacy teams into canonical rows and syncs canonical updates back to legacy tables. | This is the highest-risk coupling and needs a dedicated module without behavior drift. |
| Trait implementation | `TeamRepository for SeaOrmTeamRepository` | Exposes create/get/update/delete/list/count/member operations. | The external behavior surface should stay readable and independent from helper details. |
| Existing tests | `team_repository_tests.rs` | Covers deleted filtering, pagination, legacy visibility/backfill, inactive/invalid member handling, canonical-to-legacy sync, and cleanup. | Focused verification for this tranche. |

### Design

1. Keep `team_repository.rs` as the root module with docs, child module declarations, `SeaOrmTeamRepository`, `new`, `backend`, and `ph`.
2. Move JSON helpers and legacy/core conversion functions to `team_repository/conversions.rs`.
3. Move canonical `teams` / `team_members` SQL helpers and the non-deleted predicate to `team_repository/canonical.rs`.
4. Move legacy `um_teams` loading, backfill, canonical-to-legacy sync, and user team membership helpers to `team_repository/legacy_sync.rs`.
5. Move `#[async_trait] impl TeamRepository for SeaOrmTeamRepository` to `team_repository/repository_impl.rs`.
6. Use `pub(super)` only for helpers that must cross child-module boundaries; keep implementation details otherwise private.
7. Do not add a new public facade type or public helper API.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `team_repository.rs` | Root module keeps `SeaOrmTeamRepository::new` and backend placeholder helpers. |
| P2 | `conversions.rs` | Role, metadata, budget reset, and legacy/core conversion logic remain unchanged. |
| P3 | `canonical.rs` | Parameterized canonical SQL statements and non-deleted predicate remain unchanged. |
| P4 | `legacy_sync.rs` | Legacy backfill and membership cleanup remain covered by existing tests. |
| P5 | `repository_impl.rs` | Trait method signatures and method behavior remain unchanged. |
| P6 | file size | `wc -l src/storage/database/seaorm_db/team_repository.rs src/storage/database/seaorm_db/team_repository/*.rs` shows every touched file below 800. |
| P7 | queue count | tracked-file scan shows the remaining queue no longer includes `src/storage/database/seaorm_db/team_repository.rs`. |

## Risks

- Moving inherent impl helpers into sibling child modules can create visibility mistakes; `cargo check --all-features --locked` must prove module boundaries.
- Legacy sync has warning paths for invalid legacy data; this tranche must preserve existing warnings and must not turn hard errors into soft fallbacks.
- SQL helpers must keep backend-specific placeholders and parameter arrays to avoid injection or PostgreSQL/SQLite drift.
- The root file path must remain `team_repository.rs` so existing tests importing `super::team_repository::SeaOrmTeamRepository` still compile.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test storage::database::seaorm_db::team_repository_tests --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `team_repository.rs` and `team_repository/*.rs`

## Rollback

Revert the SeaORM team repository split and `specs/GH727` edits. No migrations,
schema changes, or runtime behavior changes are involved.

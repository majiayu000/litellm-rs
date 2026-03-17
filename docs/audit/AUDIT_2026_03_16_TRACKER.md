# Code Audit Issue Tracker — 2026-03-16

> 10-agent parallel audit across security, architecture, error handling, database, providers, performance, testing, dependencies, concurrency, and API design.
> Total findings: 103 issues. 26 GitHub Issues created. **All 26 resolved and merged.**

## Status Legend
- `queued` — Submitted to Harness, awaiting agent pickup
- `running` — Agent working on fix
- `pr` — PR created, awaiting CI/review
- `merged` — Fix merged to main
- `failed` — Agent failed, needs manual intervention
- `wontfix` — Decided not to fix

---

## P0 — Critical (Immediate Fix) — ALL MERGED

| Issue | Title | Category | PR | Status |
|-------|-------|----------|-----|--------|
| #76 | S3 storage_class config unwrap panic | Error/Panic | #126 | merged |
| #77 | Mistral transform_request unwrap panic | Error/Panic | #127 | merged |
| #78 | Vertex AI to_value().unwrap() in hot path | Error/Panic | #128 | merged |
| #79 | OpenAI from_f64().unwrap() NaN/Inf panic | Error/Panic | #125, #130 | merged |
| #80 | Password reset uses non-atomic 3-step flow | Security/TOCTOU | #129 | merged |
| #81 | CORS empty origins defaults to wildcard '*' | Security/Config | #107 | merged |

## P1 — High (This Week) — ALL MERGED

| Issue | Title | Category | PR | Status |
|-------|-------|----------|-----|--------|
| #82 | /auth/login no rate limiting | Security | #108 | merged |
| #83 | Key CRUD endpoints lack ownership (IDOR) | Security/AuthZ | #109 | merged |
| #84 | Custom API provider SSRF unprotected | Security/SSRF | #110 | merged |
| #85 | Routes defined but never mounted in App | API/Routing | #112 | merged |
| #86 | X-Request-ID dual generation, not in response | Middleware | #111 | merged |
| #87 | GET /auth/me registered as POST | API/REST | #113 | merged |
| #88 | 11 GatewayError variants → wildcard 500 | Error/HTTP | #114 | merged |
| #89 | AtomicValue::update() non-atomic race | Concurrency | #115 | merged |
| #90 | create_budget() TOCTOU race condition | Concurrency | #116 | merged |
| #91 | OpenAILikeProvider::name() = "openai_like" | Provider/Logic | #117 | merged |

## P2 — Medium (This Sprint) — ALL MERGED

| Issue | Title | Category | PR | Status |
|-------|-------|----------|-----|--------|
| #92 | Replace deprecated serde_yaml → serde_yml | Deps | #119 | merged |
| #93 | Upgrade base64 0.21 → 0.22 | Deps | #118 | merged |
| #94 | Upgrade thiserror 1.0 → 2.0 | Deps | #120 | merged |
| #95 | Remove no-op CorsMiddleware | Cleanup | #121 | merged |
| #96 | Redis write silently dropped in dual cache | Error/Silent | #122 | merged |
| #97 | stream_options silently discarded | API/Compat | #131 | merged |
| #98 | parking_lot::Mutex blocks async executor | Perf/Async | #133 | merged |
| #99 | API path prefix inconsistency /v1/ vs /api/v1/ | API/Design | #123 | merged |
| #100 | Remove deprecated core/types/config/ | Refactor | #132 | merged |
| #101 | Remove duplicate LiteLLMError/OpenAIError types | Refactor | #124 | merged |

---

## Merge Timeline

### Wave 1 — Harness Batch (2026-03-16 afternoon)
PRs #107-110, #113-116, #118, #120-124 (14 PRs merged via sequential pipeline)

### Wave 2 — Manual CI Fix (2026-03-16 evening)
PRs #111, #112, #117, #119 (4 PRs had CI failures, fixed manually with parallel agents)

### Wave 3 — Harness + Agent Hybrid (2026-03-17 overnight)
PRs #125-128 (Harness auto-created), #129-133 (parallel agents for remaining issues)

---

## Tier 1 Backlog — Agent-Fixable (ALL MERGED)

| Issue | Title | Category | PR | Status |
|-------|-------|----------|-----|--------|
| #134 | Login handler logs plaintext usernames (PII) | Security | #146 | merged |
| #135 | GatewayError 29 variants → 15 consolidated | Architecture | #160 | merged |
| #136 | conversions.rs 1210 lines → split modules | Architecture | #154 | merged |
| #137 | 39+ orphan LLMProvider implementations | Provider | #159 | merged |
| #138 | 12 providers dual-defined (catalog + standalone) | Provider | #151 | merged |
| #139 | OpenAI two `impl LLMProvider` merged | Provider | #149 | merged |
| #140 | 6 dead Provider enum variants removed | Provider | #150 | merged |
| #141 | BatchOperations references dead Database enum | Storage | #147 | merged |
| #142 | Redis max_connections parsed but never applied | Storage | #148 | merged |
| #143 | update_api_key_last_used N+1 DB writes | Perf | #153 | merged |
| #144 | SQLite fallback path hardcoded relative | Storage | #156 | merged |
| #145 | bincode unmaintained (RUSTSEC-2025-0141) | Deps | #152 | merged |

## Remaining Backlog — Needs Design Decisions

### Security (needs human review)
- API key uses bare SHA-256 (no salt/HMAC) — needs crypto design decision
- JWT uses HS256 symmetric — consider RS256 migration
- OAuth empty allowed_origins = permit all — needs policy decision
- check_permission() is stub (authN = authZ) — needs RBAC design

### Architecture (too large for single agent)
- core/ 37 sub-modules God Module — needs domain boundary design
- 5 dispatch macros × remaining variants — migrate to trait objects per MEMORY.md
- 27 files exceed 800-line limit

### Database/Storage
- Dual migration system (SeaORM vs raw sqlx) diverged schemas
- API key optimistic lock WHERE clause doesn't include version

### Testing Gaps
- 0 E2E tests, 0 HTTP mock tests, 0 concurrency tests
- Server/Routes 43% coverage, Storage 20% coverage

### Dependencies
- async-trait used in 107 files — native AFIT available on Rust 1.88
- 57 duplicate crate versions in tree

---

## Audit Source Agents

| # | Dimension | Duration | Findings |
|---|-----------|----------|----------|
| 1 | Security Vulnerabilities | 6m 8s | 19 |
| 2 | Architecture Design | 4m 19s | 8 |
| 3 | Error Handling | 8m 7s | 17 |
| 4 | Database/Storage | 5m 35s | 13 |
| 5 | Provider Consistency | 6m 41s | 13 |
| 6 | Performance Bottlenecks | 2m 45s | 8 |
| 7 | Test Coverage Gaps | 5m 8s | major gaps |
| 8 | Dependency Health | 4m 38s | 7 |
| 9 | Concurrency Safety | 3m 41s | 6 |
| 10 | API/Routing Design | 4m 5s | 12 |

## Resolution Summary

- **38/38 issues resolved** (100%)
- **38 PRs merged** (#107-133, #146-160)
- **0 open PRs, 0 open issues**
- **Wave 1-3**: 26 original audit issues (#76-101) → PRs #107-133
- **Wave 4**: 12 Tier 1 backlog issues (#134-145) → PRs #146-160
- **Fix sources**: Harness agents (18), manual parallel agents (20)
- **CI failures fixed manually**: 4 (rustfmt, route conflicts, test assertions, import paths)
- **Remaining**: 13 design-decision items (security, architecture, testing, deps) deferred to Tier 2/3

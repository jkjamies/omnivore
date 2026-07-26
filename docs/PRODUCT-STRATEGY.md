# Product Strategy

Where Omnivore stands as a product, what it should build next, and what it
should deliberately not build.

This is the strategic layer above the other planning docs:

- **[roadmap.md](roadmap.md)** — what's next, by tier
- **[future-ideas.md](future-ideas.md)** — the full backlog with complexity estimates
- **[features.md](features.md)** — what exists today, by tier
- **[CODEBASE-REVIEW.md](CODEBASE-REVIEW.md)** — known defects and the invariants behind them
  *(lands with the review stack; this link dangles until then)*

Written after a full review of the plugin, agent, and dashboard. Opinionated on
purpose — the point is to make the trade-offs arguable rather than implicit.

---

## 1. Positioning

### Against Codecov: adjacent, blocked on one feature

What teams actually gate on is **diff coverage** — "did this PR cover the lines
it changed?" — not project coverage, because project coverage barely moves and
nobody blocks a merge on a 0.2% drop.

Until that exists, Omnivore is a coverage *dashboard*, not a coverage *gate*,
and those are different purchases. A dashboard is a nice-to-have someone
installs; a gate is something a team's process depends on.

Everything needed is already in the data model. It is a diff parse plus a commit
status API call.

### Against SonarQube: stop making the claim

Sonar sells static analysis, code smells, security hotspots, and a tech-debt
number that goes in slide decks. Coverage is one tab of it. Framing Omnivore as
a Sonar replacement invites a comparison it loses on scope, and it is not the
comparison its actual strength wins.

Positioning against Sonar also implies a roadmap commitment to static analysis
and vulnerability rules — a permanent maintenance treadmill (see §5).

### The niche that is real

- **Self-hosted, genuinely small.** One ~10 MB Rust binary and a SQLite file,
  against Codecov's heavy self-hosted install and Sonar's JVM-plus-Postgres.
  Runs on a NAS. This is a real and underserved want.
- **Compose-aware filtering.** JaCoCo and Kover both report garbage on Compose
  codebases — `ComposableSingletons`, `LiveLiterals`, synthetic group functions
  all counted as uncovered code. Every Android team feels this. Nobody solves
  it. **This is the differentiator and it should lead the pitch.**
- **KMP multi-target as a first-class concept.** The `(target, source)` series
  model handles "unit + instrumented + iOS, measured by two different tools"
  properly. Kover handles KMP; its dashboard story is thin.
- **Multi-format ingestion.** lcov, llvm-cov, Go, Python, JaCoCo/Kover XML
  widens the audience well beyond Kotlin at low marginal cost.

**Recommended one-liner:** *self-hosted coverage for Android, Kotlin and KMP
that understands Compose — and ingests everything else too.*

---

## 2. The open strategic question: does the JVM agent need to exist?

This is the highest-leverage decision on the board, and it deserves a real
answer before more effort goes into instrumentation.

### The evidence

A single review pass of the agent found: branch coverage measuring the wrong
thing entirely, switch statements unmeasured, every probe recorded twice, ASM
frame computation silently skipping the most branch-heavy classes, a hardcoded
1024-probe array that would throw inside users' apps, and class-ID collisions
that merged two classes' coverage.

All of it in a tool that appeared to work and produced plausible numbers.

That is not a criticism of the code — it is the nature of the problem. JaCoCo
has been grinding on this since 2009. Bytecode coverage fails *silently and
believably*, which is the worst possible failure mode for a measurement tool: a
wrong number is indistinguishable from a right one without an oracle.

### The uncomfortable observation

The differentiator is Compose-aware **filtering**. Filtering does not require
owning **instrumentation**.

Kover already emits JaCoCo-compatible XML. The dashboard already ingests it. A
Compose-aware post-processing filter over Kover XML would deliver the same
user-visible value while deleting the riskiest part of the codebase — the part
with a decade-long correctness tail and no defensible moat.

| | Custom agent | Filter over Kover XML |
|---|---|---|
| Compose filtering | Yes | Yes |
| Correctness risk | High, permanent | Near zero (Kover's problem) |
| Maintenance | JVM/Kotlin/AGP version treadmill | Track XML schema only |
| Android instrumented tests | Custom AGP transform needed | Kover/JaCoCo already handle |
| Per-test coverage (§3) | Possible | Not possible |
| Differentiation | Little — users can't tell | Same |

The one thing the agent genuinely enables that a filter cannot is **per-test
coverage** (§3), which is a real strategic asset. So the decision reduces to:

> Is per-test coverage on the roadmap for certain? If yes, keep the agent and
> invest in verifying it. If no, or not for a year, the agent is carrying a
> large correctness liability for value users cannot perceive.

### If the agent stays

Non-negotiable: **differential testing against JaCoCo**. Run both over a corpus
of real projects and assert the numbers match where JaCoCo is trusted. Nothing
else catches this bug class — every defect above produced believable output.

The branch-semantics tests added in the review stack
(`BranchCoverageTest`) are the right template: compile real source, instrument,
execute one path, assert exact covered/total counts.

### If the agent goes

Keep `ComposeDetector` and `KotlinDetector` — that logic is the value, and it
transfers to a report filter almost unchanged.

---

## 3. Sequenced priorities

Ordered by leverage, not by effort.

### P0 — Diff coverage and PR gating

The single feature separating "dashboard" from "product". Already in
`future-ideas.md` as Medium; it should be the next thing built.

Ships as three pieces:
1. Diff coverage computation (git diff → line ranges → intersect with coverage)
2. GitHub Check Run with pass/fail (already backlogged, Small-Medium)
3. Threshold config for the gate ("new code must be ≥ 80%")

Note that "new code coverage" is also *the* metric SonarQube's clean-as-you-code
model sells. Winning that argument on coverage alone is plausible in a way that
winning on static analysis is not.

### P1 — Risk-weighted coverage (churn × coverage)

Join coverage against `git log` churn and rank files that are **uncovered *and*
frequently changed**. That is the list a team can act on, versus an alphabetical
list of untested files nobody reads.

Cheap to build, needs no LLM, and neither Codecov nor Sonar surfaces it well.
Strong candidate for the demo screenshot.

### P1 — Onboarding to a single command

Nothing kills self-hosted adoption faster than a twelve-step README. Target:
one `docker run` plus one CI snippet, working end to end, with the dashboard
telling you what is missing rather than failing silently.

The startup security-posture banner added in the review stack is the same idea
applied to configuration — make the system explain its own state.

### P2 — Components / flags for monorepos

Codecov's "flags" are what large repos adopt on. `(target, source)` is
architecturally close; a third axis, or a per-path component mapping, gets
there. Pairs naturally with the existing dependency-graph work.

### P2 — Per-test coverage → test impact analysis

Track which test covered which lines. Requires per-test probe snapshots in the
agent (see §2 — this is the agent's justification).

Unlocks **"run only the 40 tests touching this diff"**, which for a large
Android codebase is worth far more than the coverage report itself. This is the
most defensible feature on the list and the strongest reason to keep owning
instrumentation.

### P3 — Deliberately deferred

- Multi-branch comparison — wait for demand
- Custom dashboard widgets — configuration is not a feature
- Team/org grouping — low value until many projects exist

---

## 4. Agentic and LLM capabilities

### What is already commoditized

**"Generate tests for uncovered code."** Claude Code, Cursor, and Copilot all do
this today, and none of them needs Omnivore to do it. Building it buys a demo,
not a moat.

The useful move is the inverse: **make coverage available to the agents people
already use**, rather than adding another agent to the pile.

### Decision (tentative): MCP server, not in-product LLM features

**Omnivore exposes coverage to agents; it does not call models itself.**

> Own the data. Let people bring their own model.

The moat is Compose-aware, multi-target, ideally per-test coverage data — not
the model that reads it. Anything that makes Omnivore a *source of truth an
agent queries* compounds. Anything that makes it *another chat box* does not.

This supersedes the in-product AI features currently on the books:

| Currently planned | Disposition |
|---|---|
| AI-powered test suggestions (copy-to-clipboard prompts) | Superseded by MCP |
| Inline AI suggestions (dashboard calls an AI API, renders inline) | **Not planned** |
| PR-level AI test review (AI suggestions in PR comments) | **Not planned** |

Each of those requires Omnivore to hold a provider API key, own prompt
quality, absorb per-call cost, and break whenever a model is deprecated —
in exchange for a capability the user's existing tools already have. The MCP
server delivers the same outcome with none of that, because the agent doing
the reasoning is the one the developer already pays for.

Second-order benefit: it inverts the integration burden. Every new agent that
speaks MCP becomes an Omnivore client for free.

### Design sketch

**Shape.** A small standalone binary (`omnivore-mcp`) speaking MCP over stdio,
talking to the dashboard's existing REST API. Stdio because it is universally
supported by current clients and needs no hosting. A streamable-HTTP endpoint
served by the dashboard itself is the natural phase 2, once remote/hosted
agents matter — but it carries an auth design that stdio avoids.

Keep it a **thin shim over the REST API**, not a second consumer of the
database. The MCP spec is still moving; confining churn to a translation layer
means spec changes never reach the core.

**Tools — read-only to start.** Write access from an agent is a materially
different risk conversation and is not needed for the value.

| Tool | Backed by | Status |
|---|---|---|
| `list_projects()` | `GET /api/v1/projects` | exists |
| `list_series(project)` | `GET /api/v1/coverage/{id}/series` | exists |
| `coverage_summary(project, target?, source?)` | `GET /…/latest` | exists |
| `hotspots(project, limit?)` | derived from `files_json` | exists (page-only; needs an API) |
| `uncovered_lines(project, path)` | derived from `files_json` | **needs a new endpoint** |
| `coverage_for_diff(project, base, head)` | diff coverage | **blocked on P0** |

`uncovered_lines` is the one that matters. "Which lines in the file I am editing
are untested?" is the question an agent actually needs answered, and answering
it well — line ranges, not a percentage — is most of the value.

**Prerequisite: a read-scoped token.** API keys today authorize *writes* only;
reads are either open or gated by an OAuth **session cookie**, and an MCP client
has no browser session. So an instance running
`OMNIVORE_REQUIRE_LOGIN_TO_VIEW=true` currently has no way to grant a
machine read access.

That gap has to close before the MCP server is useful on a locked-down
instance. Options, cheapest first:

1. Give API keys a `read` scope and accept them on read endpoints — small, and
   fits the existing model.
2. A separate read-token type — cleaner separation, more surface.
3. Device-flow OAuth for CLI clients — best UX, most work.

Option 1 is the recommendation; it is a column and a guard, not a subsystem.

**Phasing.**

- **Phase 1** — `list_projects`, `list_series`, `coverage_summary`, `hotspots`,
  `uncovered_lines`, plus the read-scope work. Ships value on day one for
  anyone whose dashboard is already reachable.
- **Phase 2** — `coverage_for_diff`, once P0 lands. This is the one that makes
  an agent genuinely useful in a PR workflow.
- **Phase 3** — HTTP transport for hosted agents; per-test tools if §2 resolves
  in favour of keeping the agent.

### Still worth building alongside

1. **`omnivore context` CLI.** Emits uncovered lines plus surrounding source as
   a prompt-ready block. Covers tools with no MCP support, and doubles as a
   debugging aid for the MCP server itself. Small.

2. **Close the ratchet loop.** The auto-advancing floor already exists. "Agent
   writes tests until the floor rises, CI verifies, ratchet locks the gain in"
   is a complete story built entirely from shipped primitives — and with the
   MCP server it needs no new product surface at all, just documentation.

### Explicitly deferred

**Natural-language coverage queries** ("which modules regressed this sprint?").
Genuinely useful, but it needs org-level rollups in the data model first, and
with an MCP server the user's own agent can answer it by composing existing
tools. Revisit only if that proves inadequate.

### Risks

- **Spec churn.** MCP is young. Mitigated by the thin-shim constraint above.
- **Scope creep into write tools.** An agent that can mutate ratchet floors or
  delete projects is a different risk class. Read-only is a deliberate line;
  moving it should be a conscious decision, not an increment.
- **It is only as good as the data.** An agent confidently reporting wrong
  coverage is worse than no integration. This raises, not lowers, the priority
  of the verification harness in §7.

---

## 5. What not to build, and why

### Security / vulnerability scanning

Correctly identified as a large ongoing undertaking. It is worse than it looks:
rule sets need continuous updating, false positives destroy trust faster than
missing findings, and the incumbents (Snyk, Semgrep, GitHub Advanced Security)
have both funded research teams and years of tuning.

It is also a *different product* with different buyers. Coverage is bought by
engineering teams; security scanning is bought by security teams.

**Recommendation: do not build it.** If integration is wanted later, ingest
someone else's SARIF output and display it — the same multi-format ingestion
pattern that already works for coverage, with none of the rule-maintenance
burden.

### General static analysis / code smells

Same reasoning. `ktlint` and `detekt` already exist, are free, and run in CI.
Displaying their output is a reasonable future feature; competing with them is
not.

### A hosted SaaS tier — not yet

Self-hosted is the differentiator right now. Hosting means multi-tenancy, an
availability commitment, and a support load, all before product-market fit is
established. Revisit once diff coverage exists and people are actually
depending on it.

---

## 6. Packaging and tiering

The current tier split in `features.md` puts **API keys** and **admin role
separation** behind Pro. Both are security features.

Two problems:

1. Charging for authentication has a poor reputation in this market (the "SSO
   tax" discourse), and it lands worse when the gated feature is what stops
   anonymous writes.
2. It means the free tier's *default posture is open* — anyone who can reach
   the port can upload. That is a bad default to ship deliberately, and it
   makes the security documentation awkward to write.

**Recommendation:** move API keys and admin role separation to free. Gate on
scale and integration instead — the things that correlate with an organisation
having budget:

| Tier | Gate on |
|---|---|
| Free | Everything single-team: ingestion, dashboards, badges, auth, API keys, ratchet, Compose filtering |
| Pro | Diff-coverage gating, PR comments, notifications, extended retention, org rollups, per-test/test-impact |
| Enterprise | SSO/SAML, audit logs, Postgres/HA, support SLA |

This is also a cleaner story: free is complete and secure for one team; paid is
about process integration and scale.

---

## 7. Risks

### Silent wrongness — existential

A coverage tool's entire value is that its numbers are trustworthy. Every defect
found in review produced numbers that looked completely reasonable. A user who
discovers the branch percentage was measuring the wrong thing does not file a
bug — they stop trusting the tool and leave.

**Mitigation:** invest in the harness that proves numbers are right.
Differential testing against JaCoCo, exact-count assertions rather than
threshold assertions, and a corpus of real projects in CI. This should be
treated as a feature with a roadmap slot, not as test hygiene.

### `files_json` will not scale

Full per-line coverage is stored as one JSON blob per snapshot, parsed in its
entirety to render a single file's view. `find_file_across_targets`
deserializes every file of every series to find one path. At a few thousand
files this is already noticeable.

**Mitigation when it bites:** a `snapshot_files` table keyed by
`(snapshot_id, path)`. This is not a reason to reach for Postgres — SQLite is
fine with a better schema.

### Single-maintainer surface area

Two languages, a bytecode agent, an AGP transform, a web frontend, six ingest
formats. §2 is partly about reducing this. Every subsystem retained should earn
its place against a specific user-visible outcome.

---

## 8. Verdict

**Continue — but narrow.**

The dashboard is a good product filling a real gap: cheap, self-hosted,
multi-format, Compose-aware. The path to compelling is short and specific:

1. Ship **diff coverage and PR gating** — turns a dashboard into a gate
2. Lead the pitch with **Compose and KMP**, drop the SonarQube framing
3. Add the **MCP server** — cheap, timely, genuinely differentiating, and the
   agreed alternative to building LLM features into the product
4. Decide the **agent question** in §2 deliberately rather than by inertia
5. Build the **verification harness** that makes the numbers defensible

The largest risk is not competitors. It is shipping numbers that are quietly
wrong — which has already happened once, and was invisible until someone read
the code.

---

## Appendix: backlog reconciliation

Items whose status or priority this document changes. Statuses reflect the
review stack (`pr1`–`pr5`) once merged.

| Item | Was | Now |
|---|---|---|
| Diff coverage | Pro, Planned | **P0** — the gating feature |
| API rate limiting | Pro, Planned (Small) | **Built** (free, `OMNIVORE_INGEST_RATE_LIMIT`) |
| Ingest-time threshold enforcement | Planned (Small) | Partly built — ratchet warnings on ingest; needs pass/fail status |
| Merge coverage (union across targets) | Planned (Medium) | **Built** — composite now unions per-line data |
| Test-to-code mapping | Planned (Large) | **P2**, and the main justification for keeping the agent |
| GitHub Check Runs | Planned (Small-Medium) | **P0**, ships with diff coverage |
| API keys / admin roles | Pro | Recommend moving to **free** (§6) |
| Risk-weighted coverage (churn × coverage) | Not listed | **P1** — new |
| MCP server | Not listed | **P1, tentatively adopted** — the chosen direction for agent support (§4) |
| Read-scoped API token | Not listed | **New prerequisite** — API keys authorize writes only, so MCP cannot read a login-gated instance |
| `omnivore context` CLI | Not listed | **P2** — new, complements MCP for clients without it |
| AI-powered test suggestions (copy-to-clipboard) | Pro, Planned | Superseded by MCP |
| Inline AI suggestions | Enterprise, Planned | **Won't build** (§4) |
| PR-level AI test review | Enterprise, Planned | **Won't build** (§4) |
| Security / vulnerability scanning | Implied by Sonar framing | **Won't build** (§5) |
| Hosted SaaS | Implied by tiering | Deferred until diff coverage lands |

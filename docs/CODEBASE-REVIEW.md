# Omnivore codebase review

A full review of the plugin, agent, and dashboard, oriented around the current
self-hosted deployment model.

Part 1 covers the first pass: security, correctness, and build defects. Part 2
covers the second pass, which addressed everything the first pass had listed as
recommended-but-not-done — chiefly that branch coverage was not measuring
branches. Part 3 records architectural observations.

Severity is judged against the realistic threat model for a self-hosted
Omnivore: an instance on an internal network or a small VPS, ingesting from CI,
where "anyone who can reach the port" includes more people than the operators.

## Behaviour changes to be aware of

Three changes alter numbers or defaults, deliberately:

1. **Branch coverage percentages will drop.** They were counting a branch as
   covered once its condition was *reached*. See §2.1.
2. **`repo` is no longer a default OAuth scope.** Set
   `OMNIVORE_GITHUB_SCOPES=read:user,read:org,repo` if you need the source view
   for private repositories. See §1.12.
3. **`/api/v1/coverage/{id}/latest` and `/trend` now return `300` when a project
   has several coverage series** and the request does not name one. See §2.7.

---

## Part 1 — Fixed in this change

### Security

#### 1.1 Stored XSS via the embeddable trend SVG — **critical**

`routes/embed.rs` interpolated the project name into an SVG served as
`image/svg+xml`. Opened directly (which is exactly what `/embed/{id}/trend`
invites), an SVG is a *document*, not an image, and inline `<script>` executes
on the dashboard's own origin.

The project name comes from the ingest endpoint, which is unauthenticated on a
default install. So the full chain was: anyone who can POST a coverage report
gets script execution in the browser of anyone who views that project's embed —
including an operator with an admin session. `HttpOnly` protects the cookie
from being read, but not from being *used*: the payload can delete projects,
mint API keys, or exfiltrate coverage data with the victim's session.

Fixed by XML-escaping the name, clamping the caller-supplied `width`/`height`
(previously unbounded and NaN-accepting `f64`), and serving the SVG with a
restrictive CSP plus `nosniff`.

#### 1.2 Stored XSS via dependency-graph JSON — **critical**

`dependency_graph.html` embeds `graph_json()|safe` in an inline `<script>`.
`serde_json` escapes quotes and backslashes but **not** `<`, so a module named
`</script><script>alert(1)</script>` closed the script element. HTML tokenizes
`</script` before the JS parser ever sees the string, so this cannot be fixed
in JS — it has to be fixed at serialization.

Module names come from the uploaded report's dependency graph. Same
unauthenticated-uploader chain as 1.1.

Fixed with `json_for_script`, which escapes `<`, `>`, `&`, U+2028 and U+2029 as
JSON unicode escapes (they decode back to the original characters, so the data
is unchanged). Applied to `graph_json` and `all_trends_json`.

#### 1.3 Stored XSS via file paths in the file tree — **high**

`html_escape` escaped `&<>"` but not `'`, and `render_file_tree_inner`
interpolates directory paths into:

```html
onclick="toggleDir(this, '{dir_id}')"
```

A single quote in a path closed the JS string literal. File paths come from
uploaded coverage reports. Fixed by escaping `'` as `&#39;`.

#### 1.4 Any logged-in user could administer any project — **high**

`build_router` guarded the project-settings routes with
`require_login_middleware`, which checks only that *someone* is logged in. Those
routes delete projects, change the linked GitHub repo, and create project-scoped
API keys.

A `require_project_write` function with the correct semantics already existed in
`auth.rs` — it was simply never called. Wired up as
`require_project_write_middleware`: dashboard admin, or admin/maintain/write on
the project's linked repo; admin-only when no repo is linked.

#### 1.5 Self-promotion to dashboard admin — **high**

`is_dashboard_admin` fell back to "admin or maintain on *any* linked project
repo". Because ingest auto-creates projects and (see 1.6) anyone could set
`github_repo`, an outsider could upload a report, point the project at a
repository they own, and become a dashboard admin — able to change global
settings and mint global API keys.

Removed the fallback. Admin is now explicit: `OMNIVORE_ADMIN_USERS`, else
`OMNIVORE_GITHUB_ORG` owners, else nobody (with a log line explaining how to
grant it).

#### 1.6 Unauthenticated project create/update — **high**

`POST /api/v1/projects` and `PATCH /api/v1/projects/{id}` had no authentication
of any kind, even once API keys existed. `PATCH` sets `github_repo`, which
determines which repository the dashboard fetches source from and comments on —
the pivot that made 1.5 and 1.8 reachable.

Both now go through the shared `routes::api_auth` guard, enforce project-scoped
key restrictions, and validate `github_repo` as a well-formed `owner/name` slug.

#### 1.7 Confused deputy in the source-view endpoint — **high**

`/api/v1/source/{project_id}/files/{*path}` is in the open route group, but it:

1. fetches file contents from GitHub using the logged-in user's token **or the
   server's `GITHUB_TOKEN`**, then
2. caches the result in `source_cache`, keyed only by `(repo, path, ref)`, and
3. serves the cache to whoever asks next.

So an anonymous visitor could cause the server to spend its own credentials
fetching private source, and every later anonymous visitor got it from cache.

Now: when OAuth is configured the endpoint requires a session, and the server's
`GITHUB_TOKEN` is only used when OAuth is off (i.e. the operator has already
declared the instance open). Logged-in users fetch with their own token, which
is the property that makes the cache safe.

#### 1.8 PR-comment endpoint let callers spend the server's GitHub token — **high**

Ingest accepted `github_repo` and `pr_number` as query parameters and posted a
comment using the server's `GITHUB_TOKEN` if no caller token was supplied. That
token typically has write access to many repositories. Nothing tied the named
repo to the project being uploaded, so any ingest caller could make the
dashboard post arbitrary Markdown to any PR that token could reach, under the
operator's identity.

The server token is now used only when `github_repo` matches the project's
configured repository. A caller-supplied `X-GitHub-Token` is unaffected — that
is the caller spending their own authority, and it is the documented CI path.

#### 1.9 OAuth login CSRF — no `state` parameter — **high**

`/auth/login` never generated a `state` and `/auth/callback` never checked one.
An attacker could feed a victim's browser an authorization code of the
attacker's own, silently logging the victim into the attacker's GitHub identity
— after which anything the victim does on the dashboard happens in the
attacker's account.

Now: `/auth/login` mints a random state, stores it in an `HttpOnly` cookie, and
`/auth/callback` verifies it in constant time and consumes it before spending
the code.

#### 1.10 Path injection into GitHub API URLs — **medium**

`repo` was interpolated straight into `{api_base}/repos/{repo}/issues/{n}/…`
and `raw.githubusercontent.com/{repo}/{ref}/{path}`. A value like
`owner/repo/../../user` retargets the request at a different endpoint once the
HTTP client normalizes the path.

Added `omnivore_core::validation` with `is_valid_repo_slug` / `is_safe_repo_path`
(plus a git-ref check), applied at every point where these values reach a URL,
and validated on write so a bad value cannot be persisted in the first place.

#### 1.11 Auth cookies never marked `Secure` — **medium**

Session cookies were `HttpOnly` + `SameSite=Lax` but never `Secure`, so on an
HTTPS deployment any downgrade to plain HTTP leaked the session. Added
`OMNIVORE_COOKIE_SECURE`, defaulting to on when `OMNIVORE_DASHBOARD_URL` is
`https://` — the flag can't simply be hardcoded on, because that would break
login for `http://localhost:3000`.

#### 1.12 OAuth requested `repo` scope by default — **medium**

The login scope list was `read:user,read:org,repo`. `repo` grants full read
**and write** on every private repository the user can reach, and the resulting
token is stored in plaintext in SQLite for the life of the session — far more
authority than a coverage dashboard needs, and it made 1.7 much worse.

Default is now `read:user,read:org`. Operators who need the source view for
private repositories opt in with
`OMNIVORE_GITHUB_SCOPES=read:user,read:org,repo`.

**This is a deliberate behaviour change:** on an instance that relies on the
private-repo source view, that variable must be set or the view will stop
resolving private files.

#### 1.13 Authentication that fails open, invisibly — **medium**

Ingest required an API key only "if at least one key exists". Convenient for a
first run, but it fails open twice over: a fresh dashboard accepts writes from
anyone and never says so, and deleting the last key silently disables
authentication instance-wide — turning routine key rotation into a full
exposure.

The behaviour is kept for compatibility but is now explicit: it lives in
`routes/api_auth.rs`, logs a warning on every unauthenticated write, and
`OMNIVORE_REQUIRE_API_KEY=true` makes the requirement unconditional.

#### 1.14 Wide-open CORS — **medium**

`CorsLayer::permissive()` applied to every route, including ingest. Replaced
with same-origin by default and an explicit `OMNIVORE_CORS_ORIGINS` allowlist
(`*` restores the old behaviour, with a warning).

#### 1.15 Smaller hardening

- **Unbounded ingest body.** Axum's 2 MiB default is too small for real reports,
  but the endpoint is pre-auth in open mode and needs *a* ceiling. Now 32 MiB,
  configurable.
- **Database errors returned to clients.** `format!("DB error: {e}")` leaked
  schema and constraint details to unauthenticated callers. Now logged
  server-side, generic to the client.
- **Security headers.** CSP, `X-Content-Type-Options: nosniff`, and
  `Referrer-Policy` set globally as a backstop. The CSP still needs
  `'unsafe-inline'` for the pages' inline scripts, so it bounds script *origin*
  rather than eliminating injection — worth revisiting (see 2.10).
- **Gradle build cache leaked the API token.** `OmnivoreUploadTask.authToken`
  was `@get:Input`; Gradle hashes inputs into the build-cache key and records
  them in configuration-cache state and build scans, writing the credential to
  disk. Changed to `@get:Internal` — the task has no outputs and always runs, so
  the token was never needed for up-to-date checks. Also warns when uploading
  over plain HTTP to a non-loopback host.
- **Container ran as root.** The runtime image now runs as an unprivileged user.

### Correctness

#### 1.16 `ClassWriter` could not resolve application types — **high**

`instrumentClass` used `ClassWriter(COMPUTE_FRAMES)`. Frame computation must
find a common supertype whenever branches merge with different reference types
on the stack, and ASM's default `getCommonSuperClass` does that with
`Class.forName` on *its own* loader — the agent loader, which typically cannot
see application classes at all.

Every such class threw `ClassNotFoundException`, which `transform` caught and
turned into "return the class uninstrumented". The failure was silent and
precisely inverted: simple classes instrumented fine, while the branch-heavy
classes that matter most for coverage were skipped, appearing as 0% or missing
entirely.

Fixed with a `LoaderAwareClassWriter` that resolves through the transformed
class's own loader and falls back to `java/lang/Object` rather than throwing.
This is likely to *change reported coverage numbers* on real projects — that is
the fix working.

#### 1.17 Report generation crashed on stale execution data — **medium**

`OmnivoreReportTask.mergeData` copied probe arrays between files with
`for (i in data.probes.indices) probes[i] = true`. A class ID is a hash of the
class *name*, so a `.omnivore` file left over from before a recompile is keyed
identically but has a different probe count — and the copy threw
`ArrayIndexOutOfBoundsException`, failing the whole report.

Now merges the common prefix and warns with a count and a "run a clean build"
hint.

#### 1.18 Exclude patterns containing `$` silently matched nothing — **medium**

The glob-to-regex conversion, duplicated in four places, escaped only `.`:

```kotlin
pattern.replace(".", "\\.").replace("*", ".*").replace("?", ".")
```

Every other regex metacharacter passed through. The one that bites is `$`,
present in the name of every JVM inner and synthetic class: excluding
`com.example.Foo$Bar` compiled to "`com.example.Foo`, then end-of-input, then
`Bar`", which matches nothing — so the exclusion silently did not apply.
`+ ( ) [ ] { } | ^` had the same problem, and unbalanced `(` or `[` threw
`PatternSyntaxException` mid-instrumentation.

Consolidated into one `GlobPattern` object that escapes properly, caches
compiled patterns, and treats an invalid `regex:` pattern as never-matching
rather than throwing. Covered by `GlobPatternTest`.

#### 1.19 Ratchet floors advanced on any branch — **medium**

`ingest_snapshot` called `advance_ratchet_floor` on every ingest. A one-off
feature branch with unusually high coverage permanently raised the floor for the
whole project — and because the branch was never merged, raised it to a number
no main-line build could reach again, leaving the project permanently in
violation.

Now only branches on the release line advance the floor (`main`/`master` by
default, or `OMNIVORE_RATCHET_BRANCHES`). A snapshot with no branch recorded
still counts, since single-branch setups often omit it.

#### 1.20 `PRAGMA foreign_keys = ON` applied to one pooled connection

It was executed once as a query against a pool of five connections. SQLite
pragmas are per-connection, so `ON DELETE CASCADE` was live on at most one of
them, at random. Deleting a project therefore left its scoped API keys behind —
credentials outliving the thing they authorise.

Moved to `SqliteConnectOptions` (which applies per connection), and
`delete_project` now removes snapshots, API keys, and the project in one
transaction rather than relying on cascade at all.

Also set there: `journal_mode=WAL`, `synchronous=NORMAL`, and a 5s busy timeout.
With the default rollback journal, a page render reading while an ingest writes
fails outright with `SQLITE_BUSY` instead of waiting — a real symptom for a
self-hosted instance under concurrent CI uploads.

### Build and CI

#### 1.21 `docker build` could not succeed — **high**

sqlx verifies every `query!`/`query_as!` macro against a live database at
compile time. The Dockerfile created an *empty* database, so the build failed
with `no such table: projects`. The tables only appeared after the server had
run once — which cannot happen before the binary exists.

Fixed by making `crates/omnivore-core/schema.sql` the single source of truth:
`run_migrations` executes it at startup via `include_str!`, and the Dockerfile
and CI apply it to create the build database. (Guarded `ALTER TABLE`s for older
deployments stay in `run_migrations`; new columns must be added in both places.)

#### 1.22 `cargo test` could not compile — **high**

`api_tests.rs` did `include_str!("../../../../test-rigs/kmp-test-rig/build/reports/omnivore/omnivore-report.json")`
— a Gradle *build artifact*. On a clean checkout the test binary failed to
compile, so the dashboard's entire suite was un-runnable. Replaced with a
committed fixture.

#### 1.23 The agent's unit tests never ran — **medium**

`:omnivore-agent:test` failed at startup with "Failed to load JUnit Platform":
Gradle 9 no longer puts `junit-platform-launcher` on the test runtime classpath.
Added it to all three modules. `ComposeDetectorTest` has presumably not run
since the Gradle 9 upgrade.

#### 1.24 Neither component had CI

`coverage.yml` builds the KMP rig and uploads coverage; it never runs
`cargo test`, `cargo build`, `docker build`, or the plugin's own tests. That is
how 1.21, 1.22 and 1.23 all reached `main` unnoticed.

Added `.github/workflows/dashboard.yml` (build + test + docker build + health
smoke test) and `.github/workflows/plugin.yml` (Gradle build + tests).
`cargo fmt`/`clippy` are advisory for now — the tree predates any such gate and
enforcing it would fail on ~180 pre-existing diffs unrelated to any given PR.
Run `cargo fmt --all` once, then flip both to hard failures.

### Test coverage added

`api_tests.rs` gained regression tests pinning each fix: SVG escaping, dimension
clamping, script-tag breakout via dependency JSON, single-quote escaping in the
file tree, repo-slug validation, API-key enforcement, project-scope enforcement,
key revocation on project delete, and branch-gated ratchet advancement. Plus
`GlobPatternTest` and unit tests for `validation`.

Totals: dashboard 30 core + 33 API tests; plugin 67 tests. All passing.

---

## Part 2 — Second pass: correctness, aggregation, and hardening

Everything listed as "recommended, not done" in the first pass has now been
addressed. This section records what changed and why, in the same order.

### 2.1 Branch coverage now measures branches — **the central fix**

`ProbeInserter` placed a single probe *in front of* each conditional jump, so it
fired when the condition was **evaluated**. `if (x) a() else b()` reported 100%
branch coverage from a test that only ever took the `true` path — wrong in the
optimistic direction, which is the worst direction for a quality gate, and it
propagated silently into ratchet floors and PR comments.

Probes now sit on control-flow **edges**. Each conditional jump is rewritten so
both outcomes route through their own probe block:

```text
    <operands>                       <operands>
    IFEQ target        becomes       IFEQ probeTaken
    <fall-through>                   probes[notTaken] = true
                                     GOTO after
                                 probeTaken:
                                     probes[taken] = true
                                     GOTO target
                                 after:
                                     <fall-through>
```

`BranchCoverageTest` compiles real Java with the JDK compiler, instruments it,
runs it, and asserts exact covered/total counts — including the case that
motivated the work: exercising one side of an `if` yields 1 of 2, not 2 of 2.

**Expect reported branch coverage to drop** on real projects. That is the fix
working; the previous numbers were not measuring what they claimed.

### 2.2 Switch statements are instrumented

`TABLESWITCH`/`LOOKUPSWITCH` produced no probes at all, so `when` over an enum
or sealed class — ubiquitous in Kotlin — contributed nothing. Each arm now gets
a probe, plus one for `default`. Two case labels sharing a target still count as
two edges, matching JaCoCo.

### 2.3 Hit counts no longer claim precision they don't have

The agent uses boolean probes, so `hitCount` is only ever 0 or 1, yet the file
view rendered a `1x` badge on every covered line — implying an execution count
that was never measured. Counting would mean a read-modify-write on every probe:
slower, and lossy under concurrency without atomics. JaCoCo makes the same
tradeoff, so the probes stay boolean and the **UI** was fixed: a covered line
shows a check, and an exact count appears only when the value exceeds 1 (which
happens for JaCoCo XML's `ci`, where counts are real). The model documents the
semantics.

### 2.4 Class IDs use CRC64

`hash * 31 + char` accumulated into a `Long` never mixes short names into the
high bits, and names sharing a long prefix stay numerically adjacent — exactly
the shape of a package hierarchy. A collision silently merged two classes'
coverage.

IDs now use CRC64 (ECMA-182). The ID still derives from the class *name* rather
than the bytecode as JaCoCo does, because it is baked into the instrumented
`<clinit>` and the AGP path never sees the original bytes — the reasoning is
recorded in `ClassId`. Staleness is handled separately: the report task drops
classes whose execution data and probe map disagree on probe count, rather than
correlating them into coverage attributed to the wrong lines.

### 2.5 Composite coverage unions instead of double-counting

`compute_composite` summed each target's totals, so a file exercised by both unit
and instrumented tests was counted twice — a weighted average of targets rather
than "what did all the tests together cover?". Lines are now unioned from
per-line data: a line counts as covered if any target hit it, and each distinct
file contributes once.

Branch *edges* still sum, because the report carries per-file branch totals but
no per-edge identity, so one target's edges cannot be matched against another's.
`CompositeSnapshot` exposes `lines_are_union` / `branches_are_union` so this is
explicit rather than assumed.

### 2.6 Directory branch rates are weighted

A directory's rate was the arithmetic mean of its files' rates, so a 3-branch
file counted as much as a 300-branch one — and because a branchless file
reported `1.0`, a directory of straight-line code dragged genuinely
poorly-covered directories upward.

Rates are now `covered / total` across the directory's files. That required
per-file branch counts, so `FileCoverage` gained `branchesCovered` /
`branchesTotal`, plumbed through **every** parser — lcov and JaCoCo already
computed them per file and were discarding them. A branchless file now reports
`0.0` with `branchesTotal == 0`, which keeps it out of weighted rollups instead
of inflating them. Reports from older producers carry no counts, so the
unweighted mean remains as a fallback.

### 2.7 The JSON API and badge speak in series

`get_latest` meant "newest row for this project" across all `(target, source)`
series, so a project uploading both unit and instrumented coverage returned
whichever finished last, and `/trend` interleaved series into one sawtooth.

Both now take `?target=` and `?source=`. With one series the parameters are
optional; when the filters match several the endpoint returns `300 Multiple
Choices` rather than picking arbitrarily, and a new
`/api/v1/coverage/{id}/series` lists the options. The badge does the same and
renders "unknown" instead of guessing — a badge that changes meaning between CI
runs is worse than one that admits ambiguity.

### 2.8 Caches expire

`TREE_CACHE` held every repo's full file listing for the process lifetime;
`invalidate_tree_cache` existed but nothing called it, so a rename never appeared
until restart. It now has a 15-minute TTL and an LRU-ish cap.

`source_cache` rows were keyed with an empty `commit_ref` and never expired, so
the file view showed whatever was fetched first, forever — stale source rendered
against current coverage line numbers puts the gutter marks on the wrong lines.
Entries fetched against a floating ref now expire after an hour; entries pinned
to a commit SHA are immutable and never do.

### 2.9 Session tokens are encrypted at rest

`OMNIVORE_SECRET_KEY` enables ChaCha20-Poly1305 encryption of stored GitHub
tokens. AEAD rather than a stream cipher, so a tampered ciphertext fails loudly
instead of decrypting to garbage that then gets sent to GitHub.

Deliberately opt-in: a self-hosted instance must keep working after an upgrade
without the operator acting first, and auto-generating a key would be worse than
not encrypting — one held only in memory invalidates every session on restart,
and one written beside the database protects against nothing. Values are tagged,
so plaintext and ciphertext coexist and enabling encryption logs nobody out. A
session whose token cannot be decrypted is treated as absent, sending the user
back through login rather than making API calls with garbage.

### 2.10 CSRF tokens on the form endpoints

Settings, project delete, ratchet, and API-key creation were plain form POSTs
with `SameSite=Lax` as the only defence. Now a signed double-submit token: a
random value plus an HMAC, issued in a readable cookie and echoed in a hidden
field. Signing keeps it stateless — no table, and tokens survive a restart when
`OMNIVORE_SECRET_KEY` is set. Only enforced when OAuth is on; with no
authentication there is no session to ride, so requiring a token would break
`curl`-driven setup while protecting nothing.

The CSP still carries `'unsafe-inline'`, because the templates use inline
`<script>`. Moving those to files under `/static` and dropping `'unsafe-inline'`
remains worthwhile and is the one genuinely open item here.

### 2.11 Ingest is rate-limited and project creation is controllable

Ingest auto-created a project for any unseen `project_id` with no ceiling —
unbounded row creation from an endpoint that is unauthenticated in open mode.
There is now a per-client fixed-window limit (`OMNIVORE_INGEST_RATE_LIMIT`,
default 60/min) and `OMNIVORE_ALLOW_PROJECT_AUTOCREATE=false` to require that
projects be created deliberately.

The limiter keys on `X-Forwarded-For` then peer address. That header is
client-controlled, so this throttles accidents and casual abuse; it is not an
authorization boundary — the API key is.

### 2.12 Optional login-to-view

`OMNIVORE_REQUIRE_LOGIN_TO_VIEW=true` extends the login requirement over the
read-only pages and API. Health, the auth endpoints, and the API-key-authenticated
write endpoints stay reachable regardless — gating any of those would deadlock
the instance.

### 2.13 Explicit includes override the infrastructure skip list

The built-in list contains broad prefixes (`com.google.`, `com.squareup.`) and
ran *before* user includes, so anyone whose code lives under one got no coverage
and no way to override it. An explicit include now wins; an exclude still beats
an include, since the safer reading of "exclude this" is to honour it.

### 2.14 CI no longer sends the workflow token to an unpinned host

`coverage.yml` sent `secrets.GITHUB_TOKEN` to a URL from a repository
*variable* — a weaker permission to edit than a secret. The workflow now
requires `https://`, optionally pins the host via `OMNIVORE_DASHBOARD_HOST`, and
passes values through the environment rather than interpolating them into the
shell command.

### 2.15 Smaller items

- **`Content-Disposition` filenames** are sanitized; a quote in a project ID
  produced a malformed header that axum then dropped.
- **Format auto-detection** no longer treats *any* unrecognised JSON as an
  omnivore report, so a wrong-format upload says "could not detect format"
  instead of surfacing an obscure serde error.
- **`prune_permission_cache`** is called — it was dead code. A background
  maintenance task now prunes expired sessions, the permission cache, and stale
  source blobs every 30 minutes. Housekeeping failures are logged, never fatal.
- **`ExecutionDataStore.getOrCreateProbes`** grows the array when a later
  registration needs more probes. It previously returned the shorter array, so
  the extra probes wrote out of bounds and threw *inside the code under test* —
  a coverage tool must never be able to crash the program it measures.
- **`ProbeInserter.seenLines`** behaviour (first occurrence of a line per method)
  is now documented as a decision rather than left implicit.

### Additional defects found while doing the above

Three bugs surfaced during this pass that were not in the first review, all
found by the new tests:

- **Every probe was recorded twice.** `instrumentClass` called `buildProbeMap`
  and *also* passed the probe map to `ProbeInserter`, so each probe got two
  entries. Line entries collapsed on insert (keyed by line number) which hid it,
  but the analyzer counts one branch per BRANCH entry — so **reported branch
  totals were doubled**. Caught by the first edge-probe test, which expected 2
  edges and got 4.
- **The AGP path allocated a fixed 1024-probe array.** Because AGP hands the
  transform a streaming visitor, the build-time path could not know the real
  probe count when emitting `<clinit>`, and used a hardcoded "generous
  pre-allocation". Any class with more than 1024 probes would have thrown
  `ArrayIndexOutOfBoundsException` at runtime — and edge probes roughly double
  probe counts, so that ceiling was about to start being hit. The path now
  buffers into a `ClassNode` and shares the agent's instrumentation core.
- **Probe-map merging dropped `isComposable`.** `OmnivoreReportTask.mergeData`
  rebuilt entries without the flag, so `isAllMethodsComposable()` could never be
  true and the "auto-exclude pure Compose classes" filter silently never fired.
  Merging also duplicated entries when two probe files described the same class.

The duplicated instrumentation logic across the agent and AGP paths was the root
cause of two of these, so both now run through one `ClassInstrumenter`.

### Closing the gap that let the AGP path drift

Sharing `ClassInstrumenter` fixed the divergence but not the reason it went
unnoticed for so long: the build-time transform lived in the Gradle plugin
module, which is `compileOnly` against AGP. Nothing in that module can be
unit-tested without a full Android toolchain, so nothing in it was. A code path
that no test can reach will drift again.

Two changes make it reachable:

- **The logic moved to the agent module.** `BuildTimeInstrumentingVisitor` has
  no AGP types in its signature, so `omnivore-agent-tests` can drive it
  directly. `OmnivoreClassVisitorFactory` keeps only what genuinely needs AGP:
  the parameters and the `isInstrumentable` filter.
- **`BuildTimeInstrumentationTest` is a differential test.** It runs the same
  input class through both the build-time and load-time paths and asserts the
  probe maps are *identical* — not merely both plausible. Probe indices are
  positional, so a divergence means the same index refers to different source
  lines on Android and on the JVM. That produces believable wrong numbers and no
  error, which is the hardest kind of defect to notice. The test also pins the
  three things that were actually wrong before: both edges of a conditional,
  every switch arm plus `default`, and a 1201-probe class that loads and runs.

Unit tests still stand in for AGP, so `android.yml` runs
`omnivoreWriteBuildProbeMap` in the Android rig. That task depends on AGP's ASM
transform tasks, so building it makes real AGP load the real factory and hand it
every app class; the job then fails if the resulting probe map is missing or
header-only. No emulator is involved — the failure modes worth catching here
(factory fails to load, throws, or instruments nothing) all show up at build
time.

## Part 3 — Architecture notes

A few observations on how the system fits together, rather than defects.

**The two-tier auth model is the main source of surprise.** Read access is
governed by GitHub OAuth, write access by API keys, and the two are independent
— a dashboard can be login-gated for browsing yet accept anonymous uploads, or
vice versa. Both also default to open. That is defensible for "run it on your
laptop in five minutes", but the two mechanisms have different failure modes and
neither is obvious from the UI. A startup banner summarising the effective
posture ("OAuth: off — anyone can view. API keys: 0 — anyone can upload.") would
prevent most misconfiguration.

**Project identity is claimed, not assigned.** `project_id` comes from the
uploaded report, and the project is created on first sight. Everything
downstream — permissions via `github_repo`, ratchet floors, retention — hangs off
that self-asserted string. Project-scoped API keys are the right primitive here;
consider making them the default path and auto-creation the exception.

**`(target, source)` as the series key is a good call.** Letting the agent and
Kover coexist as independent series for the same target is the right model, and
it is applied consistently through storage, retention, and the HTML views. The
gap is that the JSON API and badge still think in terms of "the project's latest
snapshot" (2.7) — worth closing so the API tells the same story as the UI.

**SQLite is a fine choice for self-hosted, now that WAL is on.** The remaining
scaling pressure is `files_json`: a full per-line coverage blob per snapshot,
read and parsed in its entirety to render a single file's view.
`find_file_across_targets` deserializes every file of every series to find one
path. At a few thousand files that is already noticeable. If it becomes a
problem, the fix is a `snapshot_files` table keyed by `(snapshot_id, path)`
rather than reaching for Postgres.

**The agent's silent-failure posture works against it.** `transform` catches
every exception and returns `null` (uninstrumented); `instrumentClass` returns
`null` when `totalProbeCount == 0`; classes compiled without debug info yield no
line probes. Each is individually reasonable, but together they mean a class can
vanish from coverage for several different reasons, none of which produce more
than a stderr line. 1.16 was exactly this failure mode, and it was invisible
until the code was read. A summary at the end of `omnivoreReport` — "instrumented
412 classes, skipped 38 (12 no line numbers, 26 filtered)" — would make the next
one obvious.

## Part 4 — What is validated, and what isn't

The review touched instrumentation, aggregation, authorization, and the build,
which is most of the system. Not all of it could be exercised to the same
depth, and the difference matters more than usual here: a coverage tool that is
subtly wrong still produces numbers, so "it ran and printed something" is not
evidence. This is the standing account of what backs each claim. **Update it
when the balance changes** — an entry moving from the second list to the first
is the point of most of the CI added above.

### Two defects found while writing this section

Both are in the tests rather than the product, which is the point — a suite
that is not trustworthy cannot be evidence.

- **The API suite was flaky, about one run in ten.**
  `project_autocreate_can_be_disabled` set `OMNIVORE_ALLOW_PROJECT_AUTOCREATE`
  in the process environment while ~40 other `#[tokio::test]`s were running
  concurrently, and `Database::ingest_snapshot` read that variable on every
  call. Any test that happened to be mid-ingest got a refusal it never asked
  for. The setting is now resolved once in `Database::new` and stored on the
  instance, with `with_project_autocreate` for tests — so the closed mode can be
  exercised without changing behaviour for anyone else. (A comment in the test
  claimed it "does not run concurrently with others touching this var"; every
  ingesting test touches it.)
- **The suite was approaching the ingest rate limit.** Requests built with
  `axum::http::Request` have no peer address, so every one of them keyed to the
  same rate-limit bucket, `"unknown"`. The suite makes tens of ingest calls
  inside a single 60-second window against a default limit of 60. Nothing failed
  yet; adding a handful more ingest tests would have produced 429s that look
  like a coverage defect. The `send` helper now gives each request its own
  client identity.

### Verified by something that actually runs

| Area | What backs it |
|---|---|
| Edge-based branch coverage | `BranchCoverageTest`, `EndToEndInstrumentationTest` — classes generated with ASM, instrumented, loaded, executed, probes asserted |
| AGP build-time path | `BuildTimeInstrumentationTest` (differential against the agent path) locally; `android.yml` runs it under real AGP |
| Probe index positionality | The differential test above: identical probe maps, or the build fails |
| Dashboard aggregation, series resolution, retention | `omnivore-server/tests/api_tests.rs` against a real SQLite database |
| Authorization, CSRF, rate limiting | Unit and API tests, plus a manual pass against a running server |
| Migration of a pre-existing database | `migrates_a_database_created_by_an_older_version` — a regression test for a startup failure this review introduced and then caught |
| Ingest of every supported format | Parser tests plus the test rigs; the Go rig has been driven end-to-end into a live dashboard |
| `docker build` and a health check | `dashboard.yml` |
| Plugin compiles against real AGP | `plugin.yml` |

### Not verified, and what would verify it

- **Android instrumented tests on a device.** `android.yml` proves the transform
  runs and emits a probe map; it does not prove the on-device listener,
  the logcat extraction, or `omnivorePullCoverage` still work. An
  emulator job (`reactivecircus/android-emulator-runner`) would close this, at
  real CI cost. Until then, the `ANDROID_INSTRUMENTED` path should be treated as
  the least-exercised part of the plugin.
- **Agreement with an independent implementation.** Nothing checks Omnivore's
  numbers against JaCoCo's or Kover's on the same code. The rigs can already
  produce both (`-Pomnivore.kover`, `-Pomnivore.jacoco`), so a differential
  harness that ingests both and asserts they agree within a tolerance is
  cheap and would catch whole classes of counting error. This is the single
  highest-value test still missing.
- **A KMP end-to-end run through `omnivoreReport` in this review's tree.** CI
  does it on every push; it has not been done by hand since the branch-coverage
  rewrite.
- **Compose filtering against real Compose output.** `ComposeDetector` is tested
  against hand-written approximations of what the Compose compiler emits, not
  against its actual output, which changes between compiler versions.

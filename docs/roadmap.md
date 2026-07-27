# Omnivore Roadmap

See [features.md](features.md) for built features by tier and [future-ideas.md](future-ideas.md) for the full ideas backlog.

> **Read [PRODUCT-STRATEGY.md](PRODUCT-STRATEGY.md) first** for the reasoning
> behind the ordering below — positioning against Codecov and SonarQube, why
> diff coverage is the priority ahead of everything else, and the open question
> of whether the custom JVM agent should exist at all. This file is the *what*;
> that one is the *why*.

## Next Up (Free Tier)

- [ ] **Color-coded tags** — user-selectable colors per tag (requires structured tag format)
- [x] **Coverage trend embeds** — `/embed/{project_id}/trend` for wikis/Notion/Obsidian
- [x] **Coverage ratchet** — auto-advancing coverage floor per project, warns on regression

## Next Up (Pro Tier)

- [x] **API keys for upload auth** — token-based auth for CI uploads
- [x] **GitHub OAuth login** — admin vs viewer roles, per-user source fetching
- [ ] **Diff coverage** — coverage for only changed lines in a PR (server-side, can't be done client-side).
      **This is P0.** It is the feature separating "coverage dashboard" from
      "coverage gate", and the one teams actually block merges on. Ships with
      GitHub Check Runs and a new-code threshold.

## Next Up (Enterprise Tier)

- [ ] **SSO / SAML authentication**
- [ ] **Audit logs** — settings changes, uploads, timestamps
- ~~**Inline AI suggestions** — dashboard calls AI API, renders inline~~
      — dropped in favour of the MCP server; see
      [PRODUCT-STRATEGY.md §4](PRODUCT-STRATEGY.md). Having the dashboard hold a
      provider key, own prompt quality, and absorb per-call cost buys a
      capability the user's existing tools already have.

## Strategic items (see PRODUCT-STRATEGY.md)

Not tier-assigned yet; listed here so they aren't lost in the backlog.

- [ ] **Risk-weighted coverage** — rank files that are uncovered *and* churning,
      by joining coverage against `git log`. Needs no LLM; neither competitor
      surfaces it well.
- [ ] **MCP server** — expose coverage as tools a coding agent can query.
      **Tentatively the plan for agent support**, in place of building LLM
      features into the dashboard: Omnivore serves the data, the user's own
      agent does the reasoning. Read-only tools over the existing REST API.
      See [PRODUCT-STRATEGY.md §4](PRODUCT-STRATEGY.md) for the design sketch
      and phasing.
- [ ] **Read-scoped API token** — prerequisite for the above. API keys today
      authorize writes only; reads are open or gated by an OAuth *session
      cookie*, which a machine client cannot hold. Recommended fix is a `read`
      scope on existing keys.
- [ ] **Verification harness** — differential testing against JaCoCo over a real
      corpus. The review found several defects that produced entirely plausible
      numbers; nothing but an oracle catches that class of bug.
- [ ] **Harden the release path** — `publish.yml` publishes any `v*` tag with
      the project's signing key, with no check that the tag is on `main` and no
      environment gate. **Do this before the first tag**: it costs nothing while
      no release ritual exists, and the first tag is when the key is first used
      in anger. See [PRODUCT-STRATEGY.md §9](PRODUCT-STRATEGY.md).
- [ ] **Spend the pre-release window** — no published artifact means the probe
      format, report schema, REST API, plugin DSL, and DB schema are all still
      free to reshape. The agent question below and the `files_json` rework are
      the two items most damaged by deferring past `v0.1.0`. §9 has the list and
      a sequencing.
- [ ] **Decide the agent question** — keep the custom JVM agent, or replace it
      with a Compose-aware filter over Kover XML. Hinges on whether per-test
      coverage is genuinely on the roadmap.

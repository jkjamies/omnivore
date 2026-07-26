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
- [ ] **Inline AI suggestions** — dashboard calls AI API, renders inline

## Strategic items (see PRODUCT-STRATEGY.md)

Not tier-assigned yet; listed here so they aren't lost in the backlog.

- [ ] **Risk-weighted coverage** — rank files that are uncovered *and* churning,
      by joining coverage against `git log`. Needs no LLM; neither competitor
      surfaces it well.
- [ ] **MCP server** — expose coverage as tools a coding agent can query.
      Highest differentiation per unit of effort on the list.
- [ ] **Verification harness** — differential testing against JaCoCo over a real
      corpus. The review found several defects that produced entirely plausible
      numbers; nothing but an oracle catches that class of bug.
- [ ] **Decide the agent question** — keep the custom JVM agent, or replace it
      with a Compose-aware filter over Kover XML. Hinges on whether per-test
      coverage is genuinely on the roadmap.

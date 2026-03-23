# Omnivore Roadmap

See [FEATURES.md](FEATURES.md) for the authoritative list of built/planned features by tier and [FUTURE-IDEAS.md](FUTURE-IDEAS.md) for the full ideas backlog with complexity estimates.

## Next Up (Free Tier)

- [ ] **Coverage sparklines on projects page** — ~~planned~~ Done
- [ ] **Color-coded tags** — user-selectable colors per tag (requires structured tag format)
- [ ] **Coverage trend embeds** — `/embed/{project_id}/trend` for wikis/Notion

## Next Up (Pro Tier)

- [ ] **GitHub commit status checks** — pass/fail via Checks API on ingest
- [ ] **GitHub OAuth login** — admin vs viewer roles
- [ ] **Diff coverage** — coverage for only changed lines in a PR
- [ ] **API keys for upload auth** — token-based auth for CI uploads

## Next Up (Enterprise Tier)

- [ ] **SSO / SAML authentication**
- [ ] **Audit logs** — settings changes, uploads, timestamps
- [ ] **Inline AI suggestions** — dashboard calls AI API, renders inline

## Recently Completed

- [x] Ingest history / activity log (home + project pages)
- [x] Project tags / labels with filter bar
- [x] Project favoriting / pinning (localStorage)
- [x] Keyboard shortcuts (/ to search, Escape to clear)
- [x] System health dashboard (/health)
- [x] Dark/light theme toggle
- [x] Coverage sparklines
- [x] Search/filter on projects page
- [x] Dashboard home summary
- [x] GitHub Action for CI upload
- [x] Project delete with confirmation
- [x] Export reports (single + two-snapshot comparison)
- [x] Coverage badges for READMEs
- [x] Configurable thresholds (global + per-project)
- [x] Data retention settings
- [x] GitHub PR comments on ingest
- [x] Dependency graph visualization

# Publishing Setup — Required Steps

Before you can publish Omnivore to Maven Central and the Gradle Plugin Portal, complete these one-time setup tasks.

## 1. Sonatype OSSRH (Maven Central)

- [ ] Create a Sonatype OSSRH account at https://issues.sonatype.org
- [ ] Open a "New Project" JIRA ticket requesting `io.github.jkjamies` group ID
  - This is automatically approved since your GitHub username is `jkjamies`
- [ ] Once approved, note your OSSRH username and password

**GitHub Secrets to set:**
- `OSSRH_USERNAME` — your Sonatype JIRA username
- `OSSRH_PASSWORD` — your Sonatype JIRA password

## 2. GPG Signing Key

Maven Central requires all artifacts to be GPG-signed.

- [ ] Generate a GPG key: `gpg --full-generate-key` (RSA, 4096-bit)
- [ ] Publish to a keyserver: `gpg --keyserver keyserver.ubuntu.com --send-keys <KEY_ID>`
- [ ] Export the private key: `gpg --armor --export-secret-keys <KEY_ID>`

**GitHub Secrets to set:**
- `GPG_SIGNING_KEY` — the full armored private key output (including `-----BEGIN PGP PRIVATE KEY BLOCK-----`)
- `GPG_SIGNING_PASSWORD` — the passphrase for the key

**For local publishing**, you can instead put credentials in `~/.gradle/gradle.properties`:
```properties
ossrhUsername=your-username
ossrhPassword=your-password
signing.gnupg.keyName=<KEY_ID>
signing.gnupg.passphrase=your-passphrase
```

## 3. Gradle Plugin Portal

- [ ] Create an account at https://plugins.gradle.org
- [ ] Go to https://plugins.gradle.org/user/api-keys to generate API keys
- [ ] Note the key and secret

**GitHub Secrets to set:**
- `GRADLE_PUBLISH_KEY` — the key from the Plugin Portal
- `GRADLE_PUBLISH_SECRET` — the secret from the Plugin Portal

## 4. Version Management

Before a release:
- [ ] Remove `-SNAPSHOT` from `version` in `coverage-plugin/build.gradle.kts`
- [ ] Commit, tag: `git tag v0.1.0`
- [ ] Push tag: `git push origin v0.1.0`
- [ ] The `publish.yml` workflow triggers automatically on `v*` tags
- [ ] After release, bump version to next snapshot (e.g., `0.2.0-SNAPSHOT`)

## 5. First Release Checklist

- [ ] All secrets configured in GitHub repo settings
- [ ] OSSRH group ID approved
- [ ] GPG key published to keyserver
- [ ] Run `./gradlew publishToMavenLocal` locally to verify artifacts
- [ ] Run `./gradlew :omnivore-gradle-plugin:validatePlugins` to check plugin metadata
- [ ] Tag and push — watch the Actions run
- [ ] After Maven Central staging, log in to https://s01.oss.sonatype.org and "Close" then "Release" the staging repo (first release only; can be automated later)

## Summary of All GitHub Secrets Needed

| Secret | Source |
|---|---|
| `OSSRH_USERNAME` | Sonatype JIRA account |
| `OSSRH_PASSWORD` | Sonatype JIRA account |
| `GPG_SIGNING_KEY` | `gpg --armor --export-secret-keys` |
| `GPG_SIGNING_PASSWORD` | GPG key passphrase |
| `GRADLE_PUBLISH_KEY` | Gradle Plugin Portal API keys page |
| `GRADLE_PUBLISH_SECRET` | Gradle Plugin Portal API keys page |
| `OMNIVORE_DASHBOARD_URL` | (optional) For coverage workflow — your dashboard URL |
| `OMNIVORE_TOKEN` | (optional) For coverage workflow — dashboard auth token |

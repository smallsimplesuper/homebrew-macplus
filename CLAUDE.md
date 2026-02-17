# CLAUDE.md — Project Rules for macPlus

## Version Management

**Auto-bump the patch version** whenever any app code is modified (frontend `src/` or backend `src-tauri/src/`).

### How to bump

1. Increment the **patch** component (e.g. `0.2.0` → `0.2.1`) in all three config files **in sync**:
   - `package.json` → `"version": "X.Y.Z"`
   - `src-tauri/Cargo.toml` → `version = "X.Y.Z"`
   - `src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
2. Run `cargo generate-lockfile --manifest-path src-tauri/Cargo.toml` so `Cargo.lock` stays in sync.

### User-agent strings

The user-agent is centralized in `src-tauri/src/utils/http_client.rs` via:

```rust
pub const APP_USER_AGENT: &str = concat!("macPlus/", env!("CARGO_PKG_VERSION"));
```

This reads the version from `Cargo.toml` at compile time — no manual updates needed for user-agent strings.

### When NOT to bump

- Changes limited to `CLAUDE.md`, `.claude/`, `.gitignore`, or other non-app config files do not require a version bump.
- Dependency-only updates (lock file changes with no code changes) do not require a version bump.

## Publishing & Identity

**The only author/contributor visible on GitHub must be `smallsimplesuper`.**

### Commit rules
- NEVER include `Co-Authored-By` trailers (no Claude, no AI attribution)
- NEVER set git `user.name` or `user.email` to anything other than `smallsimplesuper`
- Before committing, verify git config: `git config user.name` must be `smallsimplesuper`
- Commit messages must not reference AI tools, Claude, or any other contributor

### What must NOT appear anywhere in commits, code, or config
- "TeamDex", "teamdex", "admin@teamdex.pro"
- "Claude", "Anthropic", "noreply@anthropic.com"
- Any personal names, personal emails, or personal GitHub usernames
- Only exception: `smallsimplesuper` and `com.macplus.app`

### Release process
1. Bump version (see Version Management above)
2. Commit with clean authorship (no co-author trailers)
3. Tag: `git tag vX.Y.Z`
4. Push: `git push origin main --tags`
5. GitHub Actions builds the DMG and creates a draft release
6. Manually publish the draft release on GitHub
7. `update-homebrew.yml` auto-updates the cask formula

# Tasks

Edit freely. Mark done with [x].

## Series on v0.17.10
- [x] P0 Handoff and task list
- [x] P1 Offline accounts (backend, command, UI). app-lib check and frontend typecheck pass; needs an on-device test on Windows
- [ ] P2 Auto update is one toggle, default off, no checks while off
- [ ] P3 Updater points at the fork's GitHub releases (endpoint, pubkey, release list)
- [ ] P4 Version picker plus confirm button, "Check for updates" button
- [ ] P5 Backup before downgrade, schema downgrade, backup check on upgrade
- [ ] P6 Import from an existing Modrinth folder (profiles, app.db, settings, accounts)
- [ ] P7 Skins page and skin calls handle offline accounts

## Needs user input
- [ ] Fork repo name (owner/repo) for releases and updater endpoint
- [ ] New minisign updater keypair; private key as GitHub Actions secret
- [ ] Release workflow for Windows builds on the fork

## Forward port (rebase the series onto each tag, fix conflicts, one patch set per tag)
- [ ] v0.18.0 v0.18.1 v0.18.2
- [ ] v0.19.0 v0.19.1 v0.19.2
- [ ] v0.20.0 v0.20.1 v0.20.2 v0.20.3 v0.20.4 v0.20.5
- [ ] v0.21.0 to v0.21.11
- [ ] v0.22.0 v0.22.1
- [ ] v0.23.0 v0.23.1 v0.23.2 v0.23.3

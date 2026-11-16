# Handoff: Modrinth App recode

Read this first. Then read TASKS.md. Keep both current; update them in their own patch.

## Goal
Recode the Modrinth App (modrinth/code) with:
- Offline accounts
- App version picker (downgrade and upgrade, one click each, confirm button)
- Auto update as a single toggle (default off, never forced, silent, no checking while off)
- Import of all profiles and database from an existing Modrinth folder

## User decisions
- Start at tag v0.17.10, then forward port to every newer tag up to the newest (v0.23.3 at time of writing).
- Windows only until the user asks for others.
- Offline accounts: username only, vanilla offline UUID, multiple accounts allowed.
- Version source: GitHub releases of the user's fork. A "Check for updates" button only checks that repo and only refreshes the picker.
- Picker: same style as the modpack version picker, plus a confirm button.
- Import: copy profiles, app.db, settings, accounts. Never move or delete the source. Nothing lost on error.
- Downgrade: back up first, then downgrade the schema (remove options the target version lacks). Upgrade: check for a backup of the newer version first; if none, update normally.
- Delivery: stacked patches via git format-patch, one patch per concern, least diff, efficiency first, it must actually work.
- Always keep this handoff in the repo and always send the task list to the user.

## Environment (sandbox without rustup)
- Rust 1.95 from conda-forge via micromamba:
  `curl -sSLO https://conda.anaconda.org/conda-forge/linux-64/micromamba-2.0.5-0.tar.bz2`, extract bin/micromamba,
  `MAMBA_ROOT_PREFIX=$PWD/root ./bin/micromamba create -y -n rs -c conda-forge "rust=1.95"`, add root/envs/rs/bin to PATH.
- pnpm: `npm install -g pnpm`.
- Build check: `SQLX_OFFLINE=true cargo check -p theseus` (app-lib). Run detached with `setsid nohup ... &`, tool calls kill children otherwise.
- The Tauri app crate (theseus_gui) needs webkit2gtk and may not build on Linux; verify app-lib, then review the app crate by reading.
- app-lib needs a `.env`: `cp packages/app-lib/.env.prod packages/app-lib/.env` (untracked).
- app-lib build.rs runs Gradle, which cannot download in the sandbox. Local only, never commit: in `build_java_jars`, if `SKIP_GRADLE` is set, create an empty `$OUT_DIR/java/libs/theseus.jar` and return. Then `SKIP_GRADLE=1 SQLX_OFFLINE=true cargo check -p theseus`.
- Frontend check: `pnpm install --filter "@modrinth/app-frontend..." --ignore-scripts --frozen-lockfile`, then in apps/app-frontend run `npx vue-tsc --noEmit` and `npx prettier --check <files>`.
- Allowed network: github.com, crates.io, npm, pypi, conda. static.rust-lang.org is blocked.
- No git push is possible from the sandbox. Deliver patches.

## Rules for code
- Use runtime `sqlx::query(...)`, not `sqlx::query!`, for new SQL. The macro needs the `.sqlx` offline cache regenerated.
- New Tauri commands go in three places: `apps/app/src/api/<plugin>.rs` handler list, the same file's command, and the command list in `apps/app/build.rs`.
- New UI strings go in `apps/app-frontend/src/locales/en-US/index.json` (alphabetical by key).
- Rust formatting: 4 spaces. Vue/JS: tabs.

## Code map
- Auth: `packages/app-lib/src/state/minecraft_auth.rs`, `packages/app-lib/src/api/minecraft_auth.rs`, `apps/app/src/api/auth.rs`, `apps/app-frontend/src/helpers/auth.js`, `apps/app-frontend/src/components/ui/AccountsCard.vue`.
- Updater: `apps/app/src/updater_impl.rs`, `apps/app/src/main.rs` (updater plugin around line 147, pending update around line 297), `apps/app/tauri-release.conf.json` (endpoint and pubkey), `apps/app-frontend/src/App.vue` (update check near line 1341), `apps/app-frontend/src/components/ui/app-update-button/`.
- Settings: `packages/app-lib/src/state/settings.rs`; `auto_download_updates` already exists (migration 20250815164653).
- DB: `packages/app-lib/src/state/db.rs`, `db_backup.rs`, migrations in `packages/app-lib/migrations/`.
- Folder layout and import sources: `packages/app-lib/src/state/dirs.rs`, `legacy_converter.rs`.

## Design notes
- Offline account = a `minecraft_users` row with refresh_token `offline`, access_token `0`, expires +100 years. UUID = MD5("OfflinePlayer:" + name) as version 3 UUID. `Credentials::is_offline()` skips all Mojang profile calls.

- Auto update: the only gate is `auto_download_updates` (null or false = off). `checkUpdates()` in App.vue reads it, skips all network checks while off and re-reads it every 5 minutes. Toggle lives in BehaviorSettings.vue.
- Fork releases: repo `mocny-gaz/crakrinth`. Tag `crak-v<version>` (for example crak-v0.17.10) so it never collides with upstream `v*` tags. Each release holds `crakrinth_<ver>_x64-setup.exe`, `.nsis.zip`, `.nsis.zip.sig` and its own `latest.json` (static Tauri updater format). P4 should point the updater at one release's `latest.json` and use a version comparator that allows any different version, which gives downgrade and upgrade with the same code. The picker list comes from `https://api.github.com/repos/mocny-gaz/crakrinth/releases`.
- Updater pubkey in `apps/app/tauri-release.conf.json` is the placeholder SET_BY_RELEASE_WORKFLOW. The workflow writes the repo variable UPDATER_PUBKEY into it. The Modrinth Windows signCommand is removed. Workflow is untested (no GitHub access from the sandbox).
- Version picker (P4): settings tab "App version" (`AppVersionSettings.vue`). "Check for updates" fetches `api.github.com/repos/mocny-gaz/crakrinth/releases`, keeps tags `crak-v*`, stores the list in localStorage key `app_versions`. Confirm button calls Tauri command `check_app_version(version)` (apps/app/src/updater_impl.rs; builds the updater against that release's latest.json with a comparator that accepts any different version), then `enqueue_update_for_installation`, `set_restart_after_pending_update(true)` and closes the window; the update installs on exit. CSP connect-src gained https://api.github.com. P5 hooks in before `enqueueUpdateForInstallation` in `install()`.
- P5 (done): `packages/app-lib/src/state/db_downgrade.rs`, called from `db.rs`. Before migrations, `prepare()` finds applied migrations this binary does not know. If any: snapshot app.db to `<backup dir>/app-db-downgrade-from-<newer>-to-<this>.db`, build the expected schema in memory from this binary's migrations, then in one transaction drop extra triggers, views, indexes, tables and columns, verify every expected table and column exists (otherwise roll back with an error naming the snapshot), delete the unknown migration rows, commit, and write `<same name>.json` listing what was removed. After migrations, `restore()` runs when the stored app version differs: if the matching `.db` and `.json` exist, it copies removed tables and columns back from the snapshot by primary key (rowid when no key); if they do not exist the upgrade is just the normal migration. The existing `db_backup.rs` still makes its own pre-migration backup on every version change. Limits: data-only migrations are not reverted; a column that cannot be dropped (key or unique) stays and is logged. Test: `SKIP_GRADLE=1 SQLX_OFFLINE=true cargo test -p theseus --lib db_downgrade`.
- Upstream workflows (theseus-build and others) use Namespace runners and fail on the fork; disable them in the fork's Actions settings.

## Branches and patches
- Base: tag v0.17.10, commit 457254f2102005f315ca000f21c8fee0b3b9c722.
- Working branch: recode/0.17.10. Patches live in `patches-out/0.17.10/`.
- Status of each patch is in TASKS.md.
- Disk note: the sandbox has about 15 GB; `rm -rf target` between builds (check needs ~3 GB, GUI check ~4 GB).
- Verified so far: P4 passes `cargo check -p theseus_gui` with and without `--features updater` (needs `apt-get install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev`, `mkdir apps/app-frontend/dist`, and ~4 GB free disk), plus vue-tsc and prettier. P1 passes `cargo check -p theseus`, `vue-tsc --noEmit` and prettier. The offline UUID algorithm matches the known vanilla value for "Notch" (b50ad385-829d-3141-a216-7e7d7539ba7f). The Tauri crate (apps/app) is not compiled in the sandbox; its P1 change is three small lines reviewed by reading.

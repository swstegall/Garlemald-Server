# Contributing to Garlemald Server

Thanks for wanting to help port FINAL FANTASY XIV 1.23b to Rust. This page is the
**step-by-step for a brand-new contributor**: from zero to a merged pull request.

New to the codebase? Read these first — you can go zero → running stack →
assigned issue → PR using only these docs:

- [`docs/architecture.md`](docs/architecture.md) — what the four binaries are and how they talk.
- [`docs/dev-environment.md`](docs/dev-environment.md) — build, run, log, and reset the stack.
- [`docs/lua-runtime.md`](docs/lua-runtime.md) — the Lua content system (most gameplay work lives here).
- [`docs/agents.md`](docs/agents.md) — optional: drive an AI coding agent through an issue.

---

## 1. Request access

Development is coordinated through the project **Discord** and a shared **GitHub
project board**.

- Join the Bahamut Discord: <https://discord.gg/GUDC2pDNGD> (development has
  moved there from the Garlemald Discord).
- Ask a maintainer (in Discord) to add you as a **collaborator** on:
  - the **Garlemald-Server** repository, and
  - the **Garlemald project board** —
    <https://github.com/users/swstegall/projects/4>.

You can read the code and open a fork-based PR without any access, but board
access is what lets you claim issues so two people don't pick up the same thing.

---

## 2. Pick an issue

On the [project board](https://github.com/users/swstegall/projects/4):

1. Choose an issue from the **Ready** column.
2. **Assign it to yourself.**
3. Move the card to **In progress**.

This signals you've taken it. If you're unsure where to start, ask in Discord for
a good first issue.

---

## 3. Set up your fork

You contribute from a fork and a feature branch off **`develop`** (the default
integration branch — see the branching model in
[`docs/RELEASING.md`](docs/RELEASING.md)).

```sh
# Fork swstegall/Garlemald-Server on GitHub, then:
git clone git@github.com:<you>/Garlemald-Server.git
cd Garlemald-Server
git remote add upstream https://github.com/swstegall/Garlemald-Server.git

# Always branch off the latest upstream develop:
git fetch upstream
git switch -c <type>/<short-slug> upstream/develop   # e.g. fix/lobby-timeout
```

Get the stack building and running once before you change anything — see
[`docs/dev-environment.md`](docs/dev-environment.md).

> **Branch off `develop`, never `main`.** `main` is the release branch; `develop`
> is where work integrates.

---

## 4. Make the change

- Match the surrounding code's style and conventions.
- **Every new Rust source file (`.rs`, including `build.rs`) needs the AGPL
  license header.** Copy it verbatim from a sibling file in the same crate — do
  not retype it. Markdown / TOML / SQL files do **not** carry the header. If you
  port code from a new upstream, add it to [`NOTICE.md`](NOTICE.md). (LandSandBoat
  is GPL-3.0 — verbatim translations trigger the combined-work rule; read
  `NOTICE.md`.)
- Add or update tests where it makes sense.

### Keep CI green

Every PR must pass six gates. Run them locally before you push — they are exactly
what CI runs (`.github/workflows/ci.yml`):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo run --locked -p content-test        # Lua quest walkthroughs (SEQ_000/005/010)
PROFILE=debug scripts/smoke-local.sh      # boot-smoke all four services
```

`--locked` means a dependency change must come with the updated `Cargo.lock`
committed. (`cargo fmt --all` will fix formatting; `cargo fmt --all --check` only
verifies it.) `scripts/test.sh` bundles the first four plus content-test;
`scripts/smoke-local.sh` is the boot smoke.

---

## 5. Open a pull request

```sh
git push -u origin <your-branch>
```

Then open a PR on GitHub:

- **Base:** `swstegall/Garlemald-Server` → **`develop`**.
- **Link the issue** (e.g. "Closes #NN") so it auto-closes on merge.
- Describe what changed and how you verified it.
- Make sure CI is green. `main` is protected and release-driven; you only ever PR
  into `develop`.

After you open the PR, move your board card to the appropriate column (e.g. **In
review**). A maintainer (or you, once CI passes and you have merge rights) merges
it into `develop`; it ships in the next `develop` → `main` release.

---

## Tips

- **Smaller PRs review faster.** If an issue is large, it's fine to split the work
  across several PRs.
- **Ask early.** A quick question in Discord beats a wrong-direction PR.
- **Logs and packet captures help reviewers.** When fixing protocol/behaviour,
  `RUST_LOG=<crate>=debug` and `GARLEMALD_PACKET_LOG_DIR=…` output (see
  [`docs/dev-environment.md`](docs/dev-environment.md)) make a bug reproducible.
- **AI agents are welcome.** See [`docs/agents.md`](docs/agents.md);
  [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md) tell the agent the house
  rules.

Welcome aboard. 🛠️

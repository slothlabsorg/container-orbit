# orbit — run Docker on a beefier machine, transparently

> Delegate your local Docker to a beefier machine on your LAN over SSH.
> Heavy builds and containers run there; published ports come straight back
> to your `localhost`. One command per machine.

Your laptop is the worst machine you own for running Docker — it's the one you
also need for everything else. `orbit` moves the engine to the powerful box
under your desk (another Mac, a gaming PC, a Linux server) without changing how
you work: `docker`, `docker compose`, and anything that respects
`docker context` keep working, and `docker run -p 8080:80` is still
`curl localhost:8080` on your laptop.

## Why it works

Docker already speaks to remote daemons over SSH (`DOCKER_HOST=ssh://`). That
covers 90% — builds, disk and RAM live on the host. `orbit` adds the two things
that make it usable every day:

1. **One-command setup per side.** Idempotent, engine-agnostic.
2. **Automatic port forwarding.** `orbit` watches the remote daemon's event
   stream and keeps `ssh -L` tunnels in sync with the set of published container
   ports — so remote containers are reachable on your local `localhost`. This is
   the piece that makes it feel transparent.

It manages a **standard `docker context`** (it does *not* wrap the `docker`
binary), so it's natively compatible with Docker Desktop, OrbStack, Rancher
Desktop and colima.

## Install

**macOS / Linux — Homebrew:**

```bash
brew install slothlabsorg/tap/container-orbit
```

**macOS / Linux — one-line script:**

```bash
curl -fsSL https://raw.githubusercontent.com/slothlabsorg/container-orbit/main/dist/install.sh | sh
```

**Windows — PowerShell:**

```powershell
irm https://raw.githubusercontent.com/slothlabsorg/container-orbit/main/dist/install.ps1 | iex
```

**From source:**

```bash
cargo build --release   # binary at target/release/orbit
```

## Quick start

The fastest path is the guided wizard — it finds the host on your LAN, sets up
the SSH key (authorizing it if needed), links, and runs an end-to-end self-test:

```bash
orbit setup            # ~2 minutes, interactive — the recommended way to start
```

Prefer to do it by hand? The individual commands still work:

```bash
# On the beefy machine (the host):
orbit host setup                # checks Docker + SSH, prints the join string

# On your laptop (the client):
orbit link user@192.168.1.42    # installs the SSH key, detects the socket, makes a context
orbit up                        # docker → host, opens the socket forward + port reconciler

docker run -d -p 8080:80 nginx  # runs on the host
curl localhost:8080             # …reachable here, automatically

orbit status                    # link, connection, forwarded ports, remote CPU/RAM
orbit down                      # restore your previous context, drop every forward
```

## Commands

| Command | Where | What it does |
|---|---|---|
| `orbit setup` | client | **Guided, zero-friction setup** — discover host, authorize key, link, up, self-test. |
| `orbit host setup` / `host init` | host | Verify Docker + SSH, detect the socket adapter, print the join string. Idempotent. |
| `orbit host add-key "<pubkey>"` | host | Authorize a client's SSH public key (when password SSH is disabled). |
| `orbit link <user@host>` | client | Install the SSH key, detect the remote socket, create the `orbit` docker context. |
| `orbit up [--foreground]` | client | Switch to the host, open the multiplexed SSH master + socket forward, start the port reconciler. |
| `orbit down` | client | Restore the previous context, close every forward and the master connection. |
| `orbit status` | client | Linked host, connection state, forwarded ports, remote resource usage. |
| `orbit ports [add\|rm <port>]` | client | List active forwards; manually add/remove a TCP forward (non-Docker services). |
| `orbit logs [-f]` | client | Show (and follow) the forwarder log. |
| `orbit service install\|uninstall\|status` | client | Run orbit at login (launchd / systemd user unit). |
| `orbit mcp` | client | Start the MCP server so AI assistants can drive orbit (see below). |
| `orbit doctor` | both | Diagnose SSH, remote daemon, forwarded socket and context — with the fix for each. |

Add `-v`, `-vv`, or `-vvv` to any command for increasingly verbose logs (every
ssh/forward action), and `--log-file <path>` to also write them to a file.

## Run it as a service

Keep the delegation + forwarding alive across logins/reboots:

```bash
orbit service install     # launchd agent (macOS) or systemd --user unit (Linux)
orbit service status
orbit service uninstall
```

## Talk to it from an AI assistant (MCP)

orbit ships a built-in [MCP](https://modelcontextprotocol.io) server, so Claude
(or any MCP client) can check status, bring orbit up/down, manage forwards, and
run `doctor` in plain language:

```bash
# Claude Code:
claude mcp add orbit -- orbit mcp
```

```jsonc
// Claude Desktop — mcpServers entry:
{ "mcpServers": { "orbit": { "command": "orbit", "args": ["mcp"] } } }
```

Interactive setup stays in your terminal — the server points the assistant at
`orbit setup` rather than trying to proxy prompts.

## How it works

- **Transport:** OpenSSH, one multiplexed master connection
  (`ControlMaster`/`ControlPath`) shared by all forwards.
- **Docker redirection:** `orbit` forwards the remote daemon socket to a local
  unix socket and points a standard `docker context` at it. (It avoids
  `ssh://` endpoints, which need the `docker` binary on the remote's
  non-interactive SSH `PATH` — a common breakage.)
- **Port reconciler:** connects to the forwarded socket with
  [`bollard`](https://crates.io/crates/bollard), subscribes to `/events`, and on
  every event recomputes the published ports and opens/cancels
  `ssh -O forward -L <port>:127.0.0.1:<port>` accordingly.

## Host adapters

`orbit` abstracts *how* the remote Docker socket is located behind a
`HostAdapter` trait:

- **`UnixSocketHost`** (macOS, Linux) — unix domain socket. **v1, complete — covers Mac→Mac.**
- **`WindowsWslHost`** — Docker socket inside WSL2 via an SSH bridge. **planned (v1.1).**
- **`WindowsNativeHost`** — named-pipe relay. **future.**

See [`docs/PLAN.md`](docs/PLAN.md) for the full design.

## License

MIT

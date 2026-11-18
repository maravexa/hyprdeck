# Agent collaboration research and proofs

## Charter and inventory

HyprDeck should let a desktop user grant an agent a specific resource, see
what the agent is doing, take over immediately, and audit the outcome. The first
workflows are supervised interaction with one application and repeatable UI
development in a disposable desktop. For acceptance, a grant must have a visible
scope and lifetime, every effect must have an attributed result or an explicit
unknown outcome, and the development loop must return rendered and textual
evidence from a known build.

The current desktop is a single Rust/SCTK Wayland client with a software
`tiny-skia` renderer, an event-driven Hyprland IPC client, and a per-output panel
model. Reuse `PanelModule`, its input routing, and its theme/layout contracts
for controls. The binary has a private reload socket and a D-Bus notification
bridge. Its workspace tests cover configuration, state, rendering, layout, IPC,
modules, and a private-bus notification integration. The IPC command socket
and modules that spawn processes already have substantial authority. They stay
on the human/desktop side of the broker boundary. The renderer has module
bounds today and needs stable control-level IDs and actions for accessibility.
See [Architecture](architecture.md) and [Development](development.md).

The test host ran Hyprland 0.56.2. The installed Hyprland portal 1.4.1 declares
ScreenCast, Screenshot, GlobalShortcuts and InputCapture. Its descriptor and
the upstream descriptor omit RemoteDesktop. Other desktop backends installed
on the same host advertise RemoteDesktop, so backend selection matters.
ScreenCast can select windows and supply PipeWire streams. RemoteDesktop
specifies a separate input session with libei/EIS support. A window stream alone
provides no verified per-window input guarantee. Hyprland's permission system
also depends on OS isolation around its IPC sockets. [ScreenCast protocol][sc],
[RemoteDesktop protocol][rd], [Hyprland portal descriptor][portal],
[Hyprland permissions][permissions].

## Recommended architecture

Keep the panel as the human control surface. Use a separate local broker with
authenticated, bounded messages and narrow adapter connections. The panel
requests grants and displays broker-observed state, including the actor,
resource, operation, expiry, last action, and pause/revoke/takeover controls.
Agent narration can appear alongside that state with its own provenance. Start
with one writer lease per surface. Read-only observers need independent grants.

| Interface | Responsibility and boundary |
| --- | --- |
| Panel to broker | Human control channel with grant issuance, pause, revoke and takeover. Receive authoritative status events and bounded history. |
| Runtime to broker | Submit an operation, session/task binding, resource instance, expected revision and approved observation destination. Derive identity from the connection. Never treat request fields as authentication. |
| Broker to adapter | Send typed operations with a bounded target and preconditions. Recheck at dispatch, cancel queued work, propagate cancellation to the adapter, and record any effect whose final outcome is unknown. |
| Adapter to application | Start with a dedicated browser profile and an origin- plus tab-scoped adapter. Later add narrowly typed application APIs or AT-SPI, then compositor capture where needed. Guard effects separately from resource access. |
| Broker to journal | Record intent and permission before dispatch. Record adapter result and independent verification separately. Store evidence references and policy decisions. |
| Runtime to provider | Negotiate capabilities and route observations through the approved egress boundary. Keep provider code, OAuth tokens and third-party plugins out of the panel and broker. |

Under the same Unix UID, peer credentials authenticate the OS user but cannot
distinguish two untrusted processes. Bind the runtime to an authenticated launch
channel or separate OS identity. Hide the live display, D-Bus, Hyprland IPC,
home directories and device handles from runtimes and providers. Use a
dedicated account, sandbox or VM with explicit mounts and egress. A workspace,
new browser profile, or private Wayland socket is useful for routing and
testing. Enforced containment also needs filesystem, process, IPC, and network
controls. MCP can expose typed broker calls after authentication and does not
grant access by itself.

Use exact lifetime identities for resource instances. A focus or structure
change advances the semantic revision. A human takeover invalidates queued
revisions and pauses the writer. Replacement, out-of-scope navigation, screen
lock and disconnect invalidate the grant. New tabs, browser chrome dialogs,
external launches, and unrelated surfaces need a separate explicit grant.
In-page dialogs need a new semantic observation and target check. A tight
adapter must recheck its scene or object at the effect boundary, especially
across focus changes. Revocation prevents subsequent dispatch and cancels
queued actions. It may be too late to reverse already accepted effects.

Use stable IDs, bounds, actions, focus, revision and incremental tree updates
above the renderer. The same model can serve panel tests, broker operations,
and AccessKit. AccessKit's Unix adapter implements AT-SPI over D-Bus. Prototype
one SCTK panel and popup with a screen reader before choosing a tree export
or extending the compositor. AT-SPI reaches applications, so a private adapter
must scope its own observations and actions. [AccessKit platform adapters][a11y].

### Journal and evidence

Use a transactional local journal for durable authority. Store task ancestry
for the collapsible tree and separate relationship edges for dependencies,
joins and retries. Journal events should distinguish requested, denied,
dispatched, started, failed, completed, verified, cancelled and unknown. Use
broker-generated operation IDs and immutable evidence references. On restart,
mark interrupted dispatches unknown and require verification or a deliberate
retry. Apply private permissions, bounded retention, and explicit retention
for any screenshot or semantic payload. Keep raw observations and credentials
out of default records. Deleting evidence can preserve a redacted provenance
entry. Export optional OpenTelemetry traces with safe identifiers. Playback
reads prior evidence and requires no application access. Re-execution obtains
a fresh grant. The fixture's line journal synchronizes each event and has no
production storage, retention, or recovery contract.

### Runtimes and providers

- **ChatGPT account inference.** The documented local/open-source preview uses
  user/workspace-bound dynamic registration, an opaque host ID, browser consent,
  and authorization code flow with PKCE. Use the token response to confirm
  `chatgpt.tokens.use.direct`. The requested plan scopes include `offline_access`
  and `resource.invoke`. This route requests eligible Responses API models and
  is independent of existing conversation history. The HTTP preview requires
  `store: false`, `stream: true` and supplied history, with hosted MCP and
  native computer use among the unsupported fields and tools. Account and
  workspace eligibility remain a runtime check. [Overview][siwc],
  [Registration][sign-in], [Preview limitations][preview].
- **Existing agent conversation.** Treat this as a separate, opt-in
  connector with explicit session authorization and approved observation
  destinations. A ChatGPT plan token cannot attach to that conversation.
- **OpenAI-compatible API and Ollama.** Negotiate tool calls, vision,
  structured output, streaming, context limits, conversation state and
  cancellation per endpoint and model. Ollama documents a subset of OpenAI
  endpoints, including stateless Responses. A loopback connection is
  insufficient to claim local inference if cloud models or proxying remain
  available. Deny provider changes outside the grant and enforce egress outside
  the runtime. [Ollama compatibility][ollama].
- **Runtime selection.** Begin with a small native dispatcher and a deterministic
  fixture. Evaluate Codex app-server separately for development tasks. Its
  stdio protocol supplies thread lifecycle, streamed events, approvals, and
  turn interruption. Use its own process sandbox and let it call only the
  broker interface needed by the task. Turn interruption and broker revocation
  are separate events. [Codex app-server][app-server].

## Uncertainty register

| Evidence | Experiment or decision still required |
| --- | --- |
| Portal interface and installed descriptor are known. | Inspect the effective backend for a supervised Wayland session. Measure window stream lifecycle and prove confinement separately before allowing any shared-window input. |
| HTTPS validates in the authorized host network when requested by the host client. | Reproduce the failure in the original browser or agent process. Identify its DNS result, route, proxy, trust store, chain, and hostname. Keep TLS validation on. Browser profile trust and API-client trust require separate tests. |
| AccessKit provides a Unix AT-SPI adapter and the panel has geometry. | Check Orca/AT-SPI discovery, focus and actions for a layer-shell panel and popup with stable IDs and a screen reader. |
| The provider documentation defines wire protocols and preview limits. | Test consent, account/workspace eligibility, cancellation, vision, streaming and local-only egress with explicit authorization. No persistent token is needed for these fixtures. |
| Upstream Hyprland has a Hyprtester integration suite. A private nested KWin parent can host the test compositor. | Measure cross-version repeatability, verify stronger OS isolation before running untrusted builds, and use a separate GPU/monitor/input validation tier. [Hyprland tests][hyprtests]. |

## Proof A · Scoped internal HTTPS application

Use a loopback-only synthetic application with a fresh CA. Run a negative TLS
test from an untrusted profile, reject a hostname mismatch, and test success
with the profile-local fixture CA. Obtain a screenshot and semantic snapshot. Approve a
synthetic operation and verify application state. Change tabs to exercise
takeover, reopen the shared tab, and reject the stale queued request. Exercise
an in-page dialog and an out-of-scope popup. Revoke the grant before a delayed
operation can dispatch and verify that its application effect is absent.

Prerequisites are Node.js, Playwright with its matching Firefox, `openssl`,
and NSS `certutil`. Run from this repository with a new output directory.

```sh
cargo build --locked -p hyprdeck-core --example scoped_session
node proofs/scoped-https.cjs target/debug/examples/scoped_session ../proof-a
```

Acceptance requires zero wrong-resource or external-destination dispatches,
zero post-revoke dispatches and no synthetic action before TLS validation.
For the next gate, collect ten runs and a p95 revocation-to-denial time under
500 ms. Store only the fixture screenshot, semantic snapshot and event
metadata. Report bytes as data volume and tokens only if actual provider usage
is available. The trusted fixture supplies focus events and controls all
selectors. A real internal application, production session lock signals,
adapter cancellation, human window capture and OS isolation remain independent
acceptance tests.

## Proof B · Development feedback loop

Build HyprDeck from an isolated checkout. Start a virtual KWin parent, nested
Hyprland, private D-Bus and the normal HyprDeck process with a fixed font,
1024×768 at scale 1, and a disposable two-workspace theme. Apply a theme patch,
request reload through normal startup, and detect a change in captured
compositor output. Inject a Wayland pointer click on the other workspace and
verify Hyprland state and the rendered active color. Preserve logs, build
identity, theme diff and three rendered checkpoints. The fixture runs only
trusted binaries and uses private session resources under the same OS user.

Requires Python 3.11+, `dbus-daemon`, `kwin_wayland`, `Hyprland`, `hyprctl`,
and `grim`. The compositor needs a working nested backend and the
capture and test input protocols inside that session.

```sh
cargo build --locked -p hyprdeck --bin hyprdeck --example proof_pointer
python3 proofs/nested-wayland.py target/debug/hyprdeck target/debug/examples/proof_pointer ../proof-b
```

For acceptance, all checkpoints must match exact expected colors and state,
the click must cause one workspace change, and logs must identify a failed
stage. The next gate is ten consecutive passes without human input, with
p95 patch-to-frame under five seconds on a warm build. Track cold-build time,
evidence bytes and any manual interventions separately. Hardware validation
covers physical displays, scaling, real input, screen lock, and suspend.

[sc]: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
[rd]: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
[portal]: https://github.com/hyprwm/xdg-desktop-portal-hyprland/blob/master/hyprland.portal
[permissions]: https://wiki.hypr.land/configuring/core/advanced-configuration/permissions/
[a11y]: https://github.com/AccessKit/accesskit
[siwc]: https://developers.openai.com/siwc/token-sharing-open-source
[sign-in]: https://developers.openai.com/siwc/token-sharing-open-source/sign-in
[preview]: https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations
[app-server]: https://learn.chatgpt.com/docs/app-server
[ollama]: https://docs.ollama.com/api/openai-compatibility
[hyprtests]: https://wiki.hypr.land/contributing-and-debugging/tests/

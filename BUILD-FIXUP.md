# BUILD-FIXUP — Static Build-Readiness Audit (R1 follow-up)

Static, grep-driven consistency audit of the last 4 Foundry feature commits
(`348c5d7` Phase 2, `e86ae30` Phase 3, `ebc462f` R1). No Rust toolchain was
available on the audit host, so this is a **read/grep verification only** — it
catches mechanical breakages a compiler would reject for trivial reasons
(missing struct fields, arity mismatches, missing module/use wiring) but cannot
catch type-inference, lifetime, trait-resolution, or borrow-checker errors.

## TL;DR

**Mechanical fixes applied: 0.** Every grep-verifiable consistency point the
authors flagged (and several adjacent ones) is already correct in the tree. The
code is unusually consistent. The items below are the residual set that only a
real `cargo build` / `cargo test` can confirm, grouped by how likely they are to
bite.

Run on a Rust host:

```sh
cargo build --workspace
cargo test  --workspace --no-run   # compile tests too (this audit covered tests)
cargo clippy --workspace
```

---

## What was verified clean (no action needed)

### Phase 2 — `TeamAgent` / `TeamAgentResponse` (`specialization`, `tier`; no Default)
- **`TeamAgent { ... }`**: 23 construction sites across `crates/**` + tests —
  **all 23 set `specialization` and `tier`.**
  (`types.rs` ×7, `session.rs` ×4, `service.rs` ×1, `service/spawn_support.rs` ×1,
  `events.rs` ×1, `scheduler/tests.rs` ×1, `prompts/{mod,lead,teammate}.rs` ×5,
  `tests/{scheduler_integration,prompts_events_integration,mcp_server_integration,e2e_team_flow,e2e_smoke}.rs` ×8 — counts include factory fns.)
- **`TeamAgentResponse { ... }`**: 9 sites in `aionui-api-types/src/team.rs` —
  **all set `specialization` + `tier`** (plus `pending_confirmations`).
- `TeamAgent::to_response{,_with_icon}` thread both fields through.
- MCP `team_spawn_agent` schema exposes `tier` (`mcp/tools.rs:92`); `SpawnAgentInput`
  and `SpawnAgentRequest` carry `tier`/`specialization`; `exec_spawn_agent`
  (`mcp/server.rs:570`) maps `role→specialization`, `tier→tier`.

### Phase 2 — `persist_spawned_agent` arity + `resolve_tier`
- `persist_spawned_agent` def (`service/spawn_support.rs:510`) takes 9 args incl.
  `specialization`, `tier`, `backend_explicit`. The **single** call site
  (`session.rs:794`) passes 9 matching args. `resolve_tier` (`session.rs:79`) and
  `resolve_tier_layered` (`spawn_support.rs:430`) call sites consistent.

### Phase 3 — `project_id` on rows / requests / filters
- **`ConversationRow { ... }`**: 18 sites — **all set `project_id`**
  (`#[sqlx(default)]` on the field; literals still set it explicitly).
- **`TeamRow { ... }`**: 4 sites — **all set `project_id`.**
- **`CreateConversationRequest { ... }`**: 4 struct-literal sites set `project_id`;
  the 4 `make_create_req()` helpers build via `serde_json::from_value` (`#[serde(default)]` covers it).
- **`CreateTeamRequest { ... }`**: 34 sites in `tests/session_service_integration.rs`
  + 1 in `guide/server.rs` — **all set `project_id`** (verified with a
  negative-lookahead multiline grep: 0 blocks missing the field).
- **`ConversationFilters { ... }`** (also gained `project_id`, derives `Default`):
  every literal in `sqlite_conversation.rs` (9) and `tests/conversation_repository.rs` (8)
  uses `..Default::default()`; the production literal (`conversation/service.rs:637`)
  sets `project_id: query.project_id` explicitly. `ListConversationsQuery.project_id` exists.
- SQL: `sqlite_team.rs` `create_team` INSERT = 12 cols / 12 binds incl `project_id`;
  `list_teams` filters on `project_id`. `sqlite_conversation.rs` `create` INSERT
  = 14 cols / 14 binds incl `project_id`; all `SELECT` use `*`/`c.*` (FromRow default-safe).
  `MessageSearchRow` has NO project_id field, matching `convert.rs::search_row_to_item`
  setting `project_id: None`.

### Phase 3 — `list_teams(project_id: Option<&str>)` signature
- Trait (`db/.../team.rs:38`), impls (`sqlite_team.rs:49`, `test_utils.rs:32`,
  `tests/common/mod.rs:30`, `session_service_integration.rs:211`), service wrapper
  (`service.rs:368`), route (`routes.rs:106` via `query.project_id.as_deref()`),
  and all `None` call sites — **all aligned.** `routes.rs` uses fully-qualified
  `axum::extract::Query` + a local `ListTeamsQuery` (no missing import).

### Phase 3 — new `project` module wiring
- DB: `models/mod.rs` (`mod project` + `pub use project::ProjectRow`),
  `repository/mod.rs` (`pub mod project`, `mod sqlite_project`, `pub use` both +
  `UpdateProjectParams`), `lib.rs` (re-exports `ProjectRow`, `UpdateProjectParams`,
  `IProjectRepository`, `SqliteProjectRepository`).
- api-types: `lib.rs:113` re-exports `CreateProjectRequest`, `ProjectListResponse`,
  `ProjectResponse`, `UpdateProjectRequest`.
- app router: `router/mod.rs` (`mod project`), `routes.rs` (`use ...project_routes`,
  `project_authenticated`, `.merge(project_authenticated)`), `state.rs`
  (`ModuleStates.project`, `build_project_state`). `project.rs` handler field shapes
  match `ProjectResponse`/`ProjectRow`/`UpdateProjectParams` (incl. `description.map(Some)`
  → `Option<Option<String>>`).

### Phase 3 — migration `010_projects.sql`
- Highest-numbered (001–010). `FOREIGN KEY (user_id) REFERENCES users(id)` matches
  `users.id TEXT PRIMARY KEY NOT NULL` in `001_initial_schema.sql`. Additive
  `ALTER TABLE … ADD COLUMN project_id` for conversations + teams with
  `ON DELETE SET NULL`. Picked up automatically by `sqlx::migrate!()` (`database.rs:28`).

### R1 — `aionui-team → aionui-assistant`, `new_with_assistant`, builtin orchestrator
- **No dependency cycle.** `aionui-team/Cargo.toml` adds `aionui-assistant`;
  `aionui-assistant` deps = common/api-types/db/auth/extension — none depend back
  on `aionui-team` (checked `aionui-extension`, `aionui-auth` Cargo.tomls).
- `TeamSessionService::new` (8 args) delegates to `new_with_assistant` (9 args, adds
  `assistant_service`) via `Arc::new_cyclic`. Test sites call `new` with 8 args;
  production `state.rs:546` calls `new_with_assistant` with 9 (incl `Some(assistant_service)`).
  `build_team_state` gained `assistant_service: Arc<AssistantService>`; its caller
  (`state.rs:207`) passes `assistant.service.clone()` (built at `state.rs:144`,
  cloned before the move into `ModuleStates` at `state.rs:221`).
- `AssistantService` methods used by team — `get(&str)`, `read_rule(&str, Option<&str>)`,
  `list()` — all exist with matching signatures; `AssistantError::NotFound(_)` exists
  and is matched. `AssistantResponse` fields used (`id, name, description, enabled,
  preset_agent_type, enabled_skills, custom_skill_names, disabled_builtin_skills,
  models`) all present. `aionui_assistant` re-exports `AssistantService`,
  `AssistantError`, `BuiltinAssistantRegistry`, `BuiltinAssistant`.
- `build_lead_prompt` (public wrapper, `prompts/mod.rs:27`) gained
  `available_assistants: &[AvailableAssistant]`. All call sites pass 4 args:
  production `session.rs:277` + tests `prompts/mod.rs` (×7) + `tests/prompts_events_integration.rs` (×3).
  `AvailableAssistant`/`LeadPromptParams` field shapes match. The 6 template
  placeholder consts in `lead.rs` are all present in `prompt_templates/lead.txt`.
- builtin `orchestrator`: `assistants.json` entry (line 1106) fields match
  `BuiltinAssistant` exactly. `rule_file = "orchestrator/orchestrator.{locale}.md"`
  resolves with `ROLE_PERSONA_LOCALE = "en-US"` to the existing file
  `crates/aionui-app/assets/builtin-assistants/orchestrator/orchestrator.en-US.md`.
  `preset-id-whitelist.json` includes `"orchestrator"`.
- lead-promotion logic (`spawn_support.rs:542` & `service.rs`): first agent in a
  lead-less team promoted to Lead; lead-less + no pinned id defaults
  `custom_agent_id` to `orchestrator`. Internally consistent.

---

## Residual checklist (compiler-only — grep cannot confirm)

### Confidence: HIGH it's fine, but only the compiler proves it

1. **`Arc::new_cyclic` closure capture** — `service.rs:125`.
   The closure moves every constructor arg (`repo`, `conversation_service`, …,
   `assistant_service`) into the struct and clones `weak` into `self_ref`. If any
   field type isn't `Send`/`Sigil`-clean for the cyclic build it surfaces here.
   Likely fix if it errors: none expected — pattern is standard.

2. **`Weak<TeamSessionService>` upgrade ergonomics** — `session.rs:257`,
   `mcp/server.rs:{454,464,508,610}`. `self.service.upgrade()` / `service.upgrade()`
   must match the `Weak` field type. Looks correct (`self_ref: Weak<TeamSessionService>`).

3. **`describe_assistant` / `list_available_assistants` / `resolve_role_persona`
   async + borrow** — `service/spawn_support.rs:{267,290,342,371}`. These iterate
   `AssistantResponse` slices and build owned `AvailableAssistant` / `RolePersona`.
   `role_skills_snapshot(&AssistantResponse)` borrows then clones — borrow-checker
   only. No grep-visible issue.

4. **`build_team_state` arg order** — `state.rs:207` vs def at `502`.
   Positional args: `(services, Some(cron…), backend_binary_path.clone(),
   services.guide_mcp_config.clone(), assistant.service.clone())`. Types must line
   up positionally (`Option<Arc<CronService>>`, `Arc<PathBuf>`,
   `Option<GuideMcpConfig>`, `Arc<AssistantService>`). Verify the
   `backend_binary_path` binding in scope at `state.rs:207` is `Arc<PathBuf>`.

### Confidence: MEDIUM — worth a targeted look if the build is red

5. **`ConversationFilters`/`ConversationRowUpdate`/`ListConversationsQuery`
   propagation** — Phase 3 added `project_id` to `ConversationFilters` and
   `ListConversationsQuery`. Audit confirmed the literals, but if any OTHER crate
   (outside `aionui-db`/`aionui-conversation`) constructs `ConversationFilters`
   with an exhaustive field list and no `..Default::default()`, it will fail.
   Grep found none, but a fresh `cargo build` is the authority.
   Fix if it errors: add `project_id: None` (or `..Default::default()`).

6. **`convert.rs` conversation→response mapping** — `conversation/convert.rs:296`
   (`row_to_response(conversation_row, …)`). `ConversationResponse` has **no**
   `project_id` field, and `row_to_response` does not read `row.project_id`, so the
   new column is intentionally dropped at the response boundary. If a frontend
   contract actually expects `project_id` on the conversation response, that's a
   FEATURE gap (not a build break) — out of scope for this audit.

7. **`sqlx::FromRow` + `SELECT *` ordering after `ALTER TABLE`** — `ProjectRow`,
   `ConversationRow`, `TeamRow`. sqlx `FromRow` maps by **column name**, not
   position, so `ADD COLUMN project_id` landing last in physical order is fine.
   `#[sqlx(default)]` covers pre-migration rows. No action expected; flagged only
   because it's the kind of thing that's invisible until a runtime query runs
   (it compiles regardless — `query_as` is not compile-time-checked here).

### Confidence: LOW — informational

8. **Two `build_teammate_prompt` functions** — `prompts/mod.rs:55`
   `(agent, team_name)` (re-exported from crate root, used in production +
   `tests/prompts_events_integration.rs`) vs `prompts/teammate.rs:131`
   `(&TeammatePromptParams)` (used by teammate.rs's own tests). Distinct module
   paths, no glob re-export collision. Intentional; no action.

9. **Pre-existing lint warnings** — per `AGENTS.md`, the workspace carries many
   lint *warnings* that are NOT failures. Judge `cargo build`/`just push` by exit
   code, not output volume.

---

## Overall confidence

**High** that the workspace compiles after (at most) addressing trivially-mechanical
compiler nits in the MEDIUM section — and a real chance it builds with **zero**
changes, since every mechanical consistency point the authors flagged is already
correct and the adjacent surfaces (project module wiring, SQL bind counts, request
DTO re-exports, the assistant dependency graph, the orchestrator manifest+rule) all
check out. The only things genuinely unverifiable without a toolchain are
borrow-checker / trait-resolution / lifetime errors inside the R1 async assistant
bridge (`service/spawn_support.rs`) and the `Arc::new_cyclic` constructor — none of
which show any grep-visible red flag.

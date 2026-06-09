use super::*;
use aionui_api_types::{AssistantResponse, BehaviorPolicy};
use aionui_common::AgentType;
use aionui_common::constants::{TEAM_CAPABLE_BACKENDS, has_mcp_capability};

/// Foundry R1: locale used when reading an Assistant (Role) rule body for
/// persona injection. The builtin manifest declares `rule_file` as
/// `rules/{id}.{locale}.md`; `en-US` is the canonical shipped locale and the
/// only one guaranteed present for every builtin Role (the Orchestrator ships
/// `en-US`). A future change can thread the team's UI locale through here.
const ROLE_PERSONA_LOCALE: &str = "en-US";

/// Foundry R1: persona fields resolved from a Role (= Assistant) for a spawn.
///
/// Produced by [`TeamSessionService::resolve_role_persona`] and folded into the
/// spawned conversation's `extra` so the existing first-message injector
/// (`agent_build_extra.rs` → `acp_assembler.rs::compose_preset_context` →
/// `first_message_injector.rs`) applies the persona with no agent-side change.
#[derive(Debug, Clone, Default)]
pub(crate) struct RolePersona {
    /// Resolved assistant id (echoed into `extra.preset_assistant_id`).
    pub assistant_id: String,
    /// Rule markdown body → `extra.preset_context`. Empty when the Role
    /// declares no rule (then no `[Assistant Rules]` block is injected).
    pub preset_context: String,
    /// Effective skills snapshot → `extra.skills`:
    /// `enabled_skills` + `custom_skill_names` − `disabled_builtin_skills`.
    pub skills: Vec<String>,
    /// The Role's `preset_agent_type`; used to derive the spawn backend when
    /// the lead did not pass an explicit backend/tier override.
    pub preset_agent_type: String,
}

/// Known ACP vendor labels. Kept in lockstep with the `agent_metadata`
/// seed in `005_agent_metadata.sql` — a caller hitting an unknown
/// vendor should trigger a schema drift discussion, not silently fall
/// through.
const ACP_VENDOR_LABELS: &[&str] = &[
    "claude",
    "codex",
    "gemini",
    "qwen",
    "codebuddy",
    "droid",
    "goose",
    "auggie",
    "kimi",
    "opencode",
    "copilot",
    "qoder",
    "vibe",
    "cursor",
    "kiro",
    "hermes",
    "snow",
];

pub(super) fn parse_agent_type(backend: &str) -> Result<AgentType, TeamError> {
    // Any registered ACP vendor label collapses to `AgentType::Acp`.
    if ACP_VENDOR_LABELS.contains(&backend) {
        return Ok(AgentType::Acp);
    }
    // Otherwise interpret as a top-level `AgentType` (e.g. "acp",
    // "nanobot", "aionrs", "remote", "openclaw-gateway").
    let quoted = format!("\"{backend}\"");
    if let Ok(agent_type) = serde_json::from_str::<AgentType>(&quoted) {
        return Ok(agent_type);
    }
    Err(TeamError::InvalidRequest(format!("unsupported backend: {backend}")))
}

/// Resolve the most permissive session mode for a given backend string.
/// Reuses `AgentType::full_auto_mode_id` from aionui-common.
pub(crate) fn resolve_full_auto_mode(backend: &str) -> &'static str {
    let agent_type = if ACP_VENDOR_LABELS.contains(&backend) {
        AgentType::Acp
    } else {
        let quoted = format!("\"{backend}\"");
        serde_json::from_str::<AgentType>(&quoted).unwrap_or(AgentType::Acp)
    };
    agent_type.full_auto_mode_id(Some(backend))
}

impl TeamSessionService {
    /// Check if a backend is allowed to participate in team mode.
    /// Hard whitelist passes immediately; then checks behavior_policy.supports_team;
    /// finally queries persisted `agent_capabilities` for MCP transport declarations.
    pub(crate) async fn is_backend_team_capable(&self, backend: &str) -> bool {
        if TEAM_CAPABLE_BACKENDS.contains(&backend) {
            return true;
        }
        let Ok(Some(row)) = self.agent_metadata_repo.find_builtin_by_backend(backend).await else {
            return false;
        };
        let bp_supports = row
            .behavior_policy
            .as_deref()
            .and_then(|s| serde_json::from_str::<BehaviorPolicy>(s).ok())
            .is_some_and(|bp| bp.supports_team);
        if bp_supports {
            return true;
        }
        let caps = row
            .agent_capabilities
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
        has_mcp_capability(caps.as_ref())
    }

    /// Return all backends currently team-capable (hard whitelist + behavior_policy + dynamically detected).
    /// Used to build the Lead prompt's `available_agent_types` list.
    pub(crate) async fn list_team_capable_backends(&self) -> Vec<(String, String)> {
        let Ok(rows) = self.agent_metadata_repo.list_all().await else {
            return TEAM_CAPABLE_BACKENDS
                .iter()
                .map(|b| (b.to_string(), capitalize(b)))
                .collect();
        };
        let mut result: Vec<(String, String)> = Vec::new();
        for row in &rows {
            if !row.enabled {
                continue;
            }
            // Use backend if present, otherwise agent_type as identifier
            let key = match row.backend.as_deref() {
                Some(b) => b.to_string(),
                None => row.agent_type.clone(),
            };

            // Check behavior_policy.supports_team (covers agents with backend=NULL like aionrs)
            let bp_supports = row
                .behavior_policy
                .as_deref()
                .and_then(|s| serde_json::from_str::<BehaviorPolicy>(s).ok())
                .is_some_and(|bp| bp.supports_team);
            if bp_supports {
                result.push((key, row.name.clone()));
                continue;
            }

            // Hard whitelist (only works when backend is present)
            if let Some(backend) = row.backend.as_deref()
                && TEAM_CAPABLE_BACKENDS.contains(&backend)
            {
                result.push((key, row.name.clone()));
                continue;
            }

            // Dynamic MCP detection
            let caps = row
                .agent_capabilities
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
            if has_mcp_capability(caps.as_ref()) {
                result.push((key, row.name.clone()));
            }
        }
        // Ensure hard whitelist entries are present even if not in DB
        for &b in TEAM_CAPABLE_BACKENDS {
            if !result.iter().any(|(bk, _)| bk == b) {
                result.push((b.to_string(), capitalize(b)));
            }
        }
        result
    }

    /// Return the `team_list_models` response built from DB rows.
    /// Falls back to the hardcoded response if the DB query fails.
    /// For internal agents (like aionrs with backend=NULL), enriches
    /// with models from the providers table.
    pub(crate) async fn list_models_from_db(&self, agent_type_filter: Option<&str>) -> serde_json::Value {
        let Ok(rows) = self.agent_metadata_repo.list_all().await else {
            return crate::mcp::tools::handle_team_list_models(&serde_json::Value::Null);
        };
        let provider_models = self.collect_provider_models().await;
        crate::mcp::tools::build_list_models_from_rows(&rows, agent_type_filter, &provider_models)
    }

    /// Collect all enabled provider model IDs grouped by provider name.
    /// Returns a flat list of model IDs for use by internal agents (aionrs).
    async fn collect_provider_models(&self) -> Vec<String> {
        let Ok(providers) = self.provider_repo.list().await else {
            return vec![];
        };
        providers
            .into_iter()
            .filter(|p| p.enabled)
            .flat_map(|p| serde_json::from_str::<Vec<String>>(&p.models).unwrap_or_default())
            .collect()
    }

    /// Find the provider ID that contains a given model name.
    /// Iterates all enabled providers and checks their models JSON array.
    pub(crate) async fn resolve_provider_for_model(&self, model: &str) -> Option<String> {
        let providers = self.provider_repo.list().await.ok()?;
        for p in providers {
            if !p.enabled {
                continue;
            }
            let models: Vec<String> = serde_json::from_str(&p.models).unwrap_or_default();
            if models.iter().any(|m| m == model) {
                return Some(p.id);
            }
        }
        None
    }

    pub(crate) async fn default_model_for_backend(&self, backend: &str) -> Option<String> {
        let row = self.agent_metadata_repo.find_builtin_by_backend(backend).await.ok()??;
        let json: serde_json::Value = serde_json::from_str(row.available_models.as_deref()?).ok()?;
        if let Some(id) = json.get("current_model_id").and_then(|v| v.as_str())
            && !id.is_empty()
        {
            return Some(id.to_owned());
        }
        let arr = json
            .get("available_models")
            .and_then(|v| v.as_array())
            .or_else(|| json.as_array())?;
        arr.first()
            .and_then(|e| e.get("id").and_then(|v| v.as_str()))
            .map(|s| s.to_owned())
    }

    pub async fn spawn_agent_in_session(
        &self,
        team_id: &str,
        caller_slot_id: &str,
        req: crate::session::SpawnAgentRequest,
    ) -> Result<TeamAgent, TeamError> {
        let entry = self
            .sessions
            .get(team_id)
            .ok_or_else(|| TeamError::SessionNotFound(team_id.into()))?;
        entry.session.spawn_agent(caller_slot_id, req).await
    }

    pub fn dispose_all(&self) {
        let keys: Vec<String> = self.sessions.iter().map(|entry| entry.key().clone()).collect();
        for key in keys {
            self.stop_session(&key);
        }
        info!("All team sessions disposed");
    }

    pub(crate) fn conversation_service_ref(&self) -> &ConversationService {
        &self.conversation_service
    }

    /// Create the conversation + persist the new agent slot for a spawn.
    ///
    /// Holds the per-team `add_agent` lock for the entirety of the
    /// read-modify-write on `teams.agents`, matching [`TeamSessionService::add_agent`]
    /// (W4-D23) so concurrent spawns cannot race and drop slots.
    ///
    /// The lock is *not* held across the process warmup step — callers
    /// (`TeamSession::spawn_agent`) wire that up separately so a slow
    /// `warmup` never stalls other spawns against the same team.
    /// Foundry R1: compute a Role's (= Assistant's) effective skills snapshot.
    ///
    /// `enabled_skills` + `custom_skill_names`, minus any
    /// `disabled_builtin_skills`. Order-preserving and de-duplicated. This is
    /// the same set the standalone assistant path injects via
    /// `conversation.extra.skills`.
    pub(crate) fn role_skills_snapshot(resp: &AssistantResponse) -> Vec<String> {
        let disabled: std::collections::HashSet<&str> =
            resp.disabled_builtin_skills.iter().map(String::as_str).collect();
        let mut out: Vec<String> = Vec::new();
        for name in resp.enabled_skills.iter().chain(resp.custom_skill_names.iter()) {
            if disabled.contains(name.as_str()) {
                continue;
            }
            if !out.iter().any(|s| s == name) {
                out.push(name.clone());
            }
        }
        out
    }

    /// Foundry R1: resolve a spawn request's `custom_agent_id` (or, failing
    /// that, `specialization`) to a Role (= Assistant) and load its persona.
    ///
    /// Returns `None` when no assistant service is wired, when neither
    /// candidate id resolves to a known assistant, or on lookup error (logged,
    /// non-fatal — the spawn proceeds as a plain backend agent). The lead's
    /// catalog only advertises ids that resolve, so the common path hits the
    /// `custom_agent_id` branch.
    pub(crate) async fn resolve_role_persona(
        &self,
        custom_agent_id: Option<&str>,
        specialization: Option<&str>,
    ) -> Option<RolePersona> {
        let svc = self.assistant_service()?;

        // Prefer an explicit `custom_agent_id`; otherwise treat the
        // `specialization` as a candidate assistant id (a Role named e.g.
        // "orchestrator"). Skip blank candidates.
        let candidates = [custom_agent_id, specialization]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        for candidate in candidates {
            let resp = match svc.get(candidate).await {
                Ok(resp) => resp,
                Err(aionui_assistant::AssistantError::NotFound(_)) => continue,
                Err(e) => {
                    warn!(candidate, error = %e, "resolve_role_persona: assistant lookup failed (non-fatal)");
                    continue;
                }
            };

            // Rule body is best-effort: an empty rule simply means no
            // `[Assistant Rules]` block is injected downstream.
            let preset_context = match svc.read_rule(candidate, Some(ROLE_PERSONA_LOCALE)).await {
                Ok(body) => body,
                Err(e) => {
                    warn!(candidate, error = %e, "resolve_role_persona: read_rule failed; injecting persona without rule body");
                    String::new()
                }
            };

            return Some(RolePersona {
                assistant_id: resp.id.clone(),
                preset_context,
                skills: Self::role_skills_snapshot(&resp),
                preset_agent_type: resp.preset_agent_type.clone(),
            });
        }
        None
    }

    /// Foundry R1: build the Role (= Assistant) catalog for the lead prompt's
    /// "Available Preset Assistants for Spawning" section. Returns every
    /// enabled assistant mapped to the prompt's [`AvailableAssistant`] shape.
    /// Empty when no assistant service is wired or the list call fails
    /// (degrades to today's behaviour: the section is omitted).
    pub(crate) async fn list_available_assistants(&self) -> Vec<crate::prompts::lead::AvailableAssistant> {
        let Some(svc) = self.assistant_service() else {
            return Vec::new();
        };
        let list = match svc.list().await {
            Ok(list) => list,
            Err(e) => {
                warn!(error = %e, "list_available_assistants: assistant list failed (non-fatal)");
                return Vec::new();
            }
        };
        list.iter()
            .filter(|a| a.enabled)
            .map(|a| crate::prompts::lead::AvailableAssistant {
                custom_agent_id: a.id.clone(),
                name: a.name.clone(),
                backend: a.preset_agent_type.clone(),
                description: a.description.clone().unwrap_or_default(),
                skills: Self::role_skills_snapshot(a),
            })
            .collect()
    }

    /// Foundry R1: real `team_describe_assistant` implementation. Looks the
    /// assistant up by `custom_agent_id` (the MCP arg) via the wired
    /// `AssistantService` and renders a human-readable card (name, backend,
    /// description, skills, example prompts). Returns the not-found text when
    /// the id is absent/blank/unknown or no service is wired — preserving the
    /// stub's contract for callers that depend on it.
    pub async fn describe_assistant(&self, args: &serde_json::Value) -> String {
        let id = args
            .get("custom_agent_id")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(id) = id else {
            return "Preset assistant not found".to_owned();
        };
        let Some(svc) = self.assistant_service() else {
            return "Preset assistant not found".to_owned();
        };
        let resp = match svc.get(id).await {
            Ok(resp) => resp,
            Err(_) => return "Preset assistant not found".to_owned(),
        };

        let skills = Self::role_skills_snapshot(&resp);
        let mut out = String::with_capacity(512);
        out.push_str(&format!("# {} (`{}`)\n", resp.name, resp.id));
        out.push_str(&format!("- backend: `{}`\n", resp.preset_agent_type));
        if let Some(desc) = resp.description.as_deref().filter(|d| !d.is_empty()) {
            out.push_str(&format!("- description: {desc}\n"));
        }
        if !skills.is_empty() {
            out.push_str(&format!("- skills: {}\n", skills.join(", ")));
        }
        if !resp.models.is_empty() {
            out.push_str(&format!("- models: {}\n", resp.models.join(", ")));
        }
        if !resp.prompts.is_empty() {
            out.push_str("\n## Example tasks\n");
            for p in &resp.prompts {
                out.push_str(&format!("- {p}\n"));
            }
        }
        out.push_str(
            "\nTo spawn this assistant, call `team_spawn_agent` with \
             `custom_agent_id` set to its id above. The agent type is derived \
             from its backend automatically.",
        );
        out
    }

    /// Foundry R1 (Task 5): layered tier→(backend, model) resolution.
    ///
    /// Precedence (first hit wins):
    ///   (a) a settings-backed tier→(backend, model) map — **flagged TODO**:
    ///       the team service has no settings repo wired yet, so this layer is
    ///       a no-op hook today. When a `SettingsService`/tier-map repo is
    ///       threaded in, resolve it here BEFORE (b)/(c).
    ///   (b) the resolved Role/Assistant's `models[]`: backend from the Role's
    ///       `preset_agent_type`, model picked by tier via
    ///       [`crate::session::tier_model_from_role_models`].
    ///   (c) the hardcoded default map [`crate::session::resolve_tier`]
    ///       (final fallback).
    ///
    /// `custom_agent_id`/`specialization` identify the Role for layer (b).
    /// Returns `None` only when the tier is unknown to every layer.
    pub(crate) async fn resolve_tier_layered(
        &self,
        tier: &str,
        custom_agent_id: Option<&str>,
        specialization: Option<&str>,
    ) -> Option<(String, String)> {
        let tier_trimmed = tier.trim();
        if tier_trimmed.is_empty() {
            return None;
        }

        // (a) Settings-backed map — flagged TODO; no settings source on the
        // team service yet. Intentionally left as a hook so the precedence is
        // documented and a future wiring slots in here without touching
        // callers. When added, return early on a hit.
        // Foundry R1 TODO: read tier map from settings and resolve here first.

        // (b) Role's own curated `models[]`.
        if let Some(svc) = self.assistant_service() {
            let candidates = [custom_agent_id, specialization]
                .into_iter()
                .flatten()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            for candidate in candidates {
                if let Ok(resp) = svc.get(candidate).await
                    && let Some(model) = crate::session::tier_model_from_role_models(tier_trimmed, &resp.models)
                {
                    let backend = if resp.preset_agent_type.trim().is_empty() {
                        "claude".to_owned()
                    } else {
                        resp.preset_agent_type.clone()
                    };
                    return Some((backend, model));
                }
            }
        }

        // (c) Hardcoded default map — final fallback.
        crate::session::resolve_tier(tier_trimmed)
    }

    /// Foundry R1: resolve the backend a Role (= Assistant) implies, without
    /// reading its rule body. Lightweight peek used by `spawn_agent` so the
    /// backend-capability gate validates the *actual* backend that the persona
    /// bridge in `persist_spawned_agent` will use. Returns the trimmed,
    /// non-empty `preset_agent_type` of the first resolving candidate.
    pub(crate) async fn resolve_role_backend(
        &self,
        custom_agent_id: Option<&str>,
        specialization: Option<&str>,
    ) -> Option<String> {
        let svc = self.assistant_service()?;
        let candidates = [custom_agent_id, specialization]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        for candidate in candidates {
            match svc.get(candidate).await {
                Ok(resp) => {
                    let t = resp.preset_agent_type.trim();
                    if !t.is_empty() {
                        return Some(t.to_owned());
                    }
                }
                Err(aionui_assistant::AssistantError::NotFound(_)) => continue,
                Err(e) => {
                    warn!(candidate, error = %e, "resolve_role_backend: assistant lookup failed (non-fatal)");
                    continue;
                }
            }
        }
        None
    }

    // Foundry: Phase 2 (roles + capability tiers) — added `specialization`/`tier`
    // Foundry R1: added `backend_explicit` so the Role's `preset_agent_type`
    // can drive the spawn backend only when the lead did not pin one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn persist_spawned_agent(
        &self,
        team_id: &str,
        user_id: &str,
        name: String,
        backend: String,
        model: String,
        custom_agent_id: Option<String>,
        specialization: Option<String>,
        tier: Option<String>,
        // Foundry R1: true when the lead passed an explicit `agent_type` or a
        // resolvable `tier` (i.e. the backend was deliberately chosen). When
        // false, a resolved Role's `preset_agent_type` may override `backend`.
        backend_explicit: bool,
    ) -> Result<TeamAgent, TeamError> {
        let lock = self
            .add_agent_locks
            .entry(team_id.to_owned())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone();
        let _guard = lock.lock().await;

        let row = self
            .repo
            .get_team(team_id)
            .await?
            .ok_or_else(|| TeamError::TeamNotFound(team_id.into()))?;
        let mut team = Team::from_row(&row)?;

        // Foundry R1 (lazy session): the first instance spawned into a
        // lead-less project session is promoted to Lead. Decided here so the
        // promotion can also drive the default lead persona below.
        let has_lead = team.agents.iter().any(|a| a.role == TeammateRole::Lead);
        let role = if has_lead {
            TeammateRole::Teammate
        } else {
            TeammateRole::Lead
        };

        // Foundry R1: the Orchestrator is the default lead persona. When this
        // spawn is promoted to Lead and the caller did not pin a Role, default
        // `custom_agent_id` to `orchestrator` so the persona bridge injects the
        // lead delegation rule. A non-lead spawn keeps its caller-supplied id.
        let custom_agent_id = match (&role, custom_agent_id) {
            (TeammateRole::Lead, None) => Some("orchestrator".to_owned()),
            (_, other) => other,
        };

        // Foundry R1: Role(assistant)->spawn persona bridge.
        // Resolve the Role from `custom_agent_id` (or `specialization`) and,
        // when present, (a) derive the backend from its `preset_agent_type`
        // unless the lead pinned one, and (b) fold persona fields into `extra`
        // so the existing first-message injector applies them.
        let persona = self
            .resolve_role_persona(custom_agent_id.as_deref(), specialization.as_deref())
            .await;
        let backend = match (&persona, backend_explicit) {
            // Role resolved, no explicit lead override, and the Role declares a
            // non-empty agent type → the Role's persona dictates the backend.
            (Some(p), false) if !p.preset_agent_type.trim().is_empty() => p.preset_agent_type.clone(),
            _ => backend,
        };

        let agent_type = parse_agent_type(&backend)?;
        let provider_id = if agent_type == AgentType::Aionrs {
            self.resolve_provider_for_model(&model).await.unwrap_or(backend.clone())
        } else {
            backend.clone()
        };
        // Top-level `model` is aionrs-only per spec 2026-05-12; for other
        // agent types the model/provider ride along in `extra`.
        let (top_level_model, mut extra) = if agent_type == AgentType::Aionrs {
            (
                Some(ProviderWithModel {
                    provider_id,
                    model: model.clone(),
                    use_model: None,
                }),
                serde_json::json!({
                    "teamId": team_id,
                    "backend": backend,
                }),
            )
        } else {
            (
                None,
                serde_json::json!({
                    "teamId": team_id,
                    "backend": backend,
                    "provider_id": provider_id,
                    "current_model_id": model.clone(),
                }),
            )
        };

        // Foundry R1: inject the Role persona into `extra`. These are exactly
        // the keys the standalone assistant path sets and that
        // `AcpBuildExtra`/`AionrsBuildExtra` deserialize:
        //   - `preset_context`   → first-message `[Assistant Rules]` block
        //   - `skills`           → skills index resolved by the injector
        //   - `preset_assistant_id` → provenance for downstream tooling
        // Only `preset_context` rides the agent build path today; `skills` and
        // `preset_assistant_id` are read by the same pipeline. Empty fields are
        // omitted so a Role without a rule/skills behaves like a plain spawn.
        if let Some(p) = &persona
            && let Some(obj) = extra.as_object_mut()
        {
            obj.insert("preset_assistant_id".into(), serde_json::json!(p.assistant_id));
            if !p.preset_context.trim().is_empty() {
                obj.insert("preset_context".into(), serde_json::json!(p.preset_context));
            }
            if !p.skills.is_empty() {
                obj.insert("skills".into(), serde_json::json!(p.skills));
            }
        }
        let conv_req = CreateConversationRequest {
            r#type: agent_type,
            name: Some(name.clone()),
            model: top_level_model,
            source: None,
            channel_chat_id: None,
            extra,
            // Foundry: Phase 3 (multi-project) — spawned agent inherits the team's project.
            project_id: row.project_id.clone(),
        };
        let conv = self
            .conversation_service
            .create(user_id, conv_req)
            .await
            .map_err(TeamError::from_conversation_create)?;

        // `role` (Lead vs Teammate) was decided at the top of this method
        // (lazy-session promotion) so it could also drive the default lead
        // persona; reuse it here for the persisted agent.
        let agent = TeamAgent {
            slot_id: generate_id(),
            name,
            role,
            conversation_id: conv.id,
            backend,
            model,
            custom_agent_id,
            status: None,
            conversation_type: None,
            cli_path: None,
            // Foundry: Phase 2 (roles + capability tiers)
            specialization,
            tier,
        };

        team.agents.push(agent.clone());
        let agents_json = serde_json::to_string(&team.agents)?;
        // When this spawn was promoted to Lead, also record it as the team's
        // `lead_agent_id`; otherwise leave the existing lead pointer untouched.
        let lead_agent_id = if role == TeammateRole::Lead {
            Some(agent.slot_id.clone())
        } else {
            None
        };
        self.repo
            .update_team(
                team_id,
                &UpdateTeamParams {
                    name: None,
                    agents: Some(agents_json),
                    lead_agent_id,
                },
            )
            .await?;

        Ok(agent)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_type_known_backends() {
        assert_eq!(parse_agent_type("acp").unwrap(), AgentType::Acp);
        assert_eq!(parse_agent_type("nanobot").unwrap(), AgentType::Nanobot);
        assert_eq!(parse_agent_type("remote").unwrap(), AgentType::Remote);
        assert_eq!(parse_agent_type("aionrs").unwrap(), AgentType::Aionrs);
    }

    #[test]
    fn parse_agent_type_unknown_backend_returns_error() {
        let err = parse_agent_type("unknown").unwrap_err();
        assert!(matches!(err, TeamError::InvalidRequest(_)));
    }

    #[test]
    fn parse_agent_type_openclaw_gateway() {
        assert_eq!(
            parse_agent_type("openclaw-gateway").unwrap(),
            AgentType::OpenclawGateway
        );
    }
}

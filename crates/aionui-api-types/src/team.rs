use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// A. Team management — Request DTOs
// ---------------------------------------------------------------------------

/// Input for a single agent when creating a team or adding an agent.
///
/// Each agent gets its own conversation; the first agent in a create
/// request becomes the team lead.
///
/// When `conversation_id` is supplied the existing conversation is adopted
/// rather than creating a new one (single-chat → team-chat handoff).
#[derive(Debug, Clone, Deserialize)]
pub struct TeamAgentInput {
    pub name: String,
    pub role: String,
    pub backend: String,
    pub model: String,
    #[serde(default)]
    pub custom_agent_id: Option<String>,
    /// Adopt an existing conversation instead of creating a new one.
    /// When present the conversation's `extra` is updated with `teamId`
    /// and `backend`; no new conversation row is written.
    #[serde(default)]
    pub conversation_id: Option<String>,
}

/// Request body for `POST /api/teams`.
///
/// Creates a team with the given name and agent list.
/// The first agent in the array is designated as the lead.
#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub agents: Vec<TeamAgentInput>,
    #[serde(default)]
    pub workspace: Option<String>,
}

/// Request body for `PATCH /api/teams/:id/name`.
#[derive(Debug, Deserialize)]
pub struct RenameTeamRequest {
    pub name: String,
}

// ---------------------------------------------------------------------------
// B. Agent management — Request DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /api/teams/:id/agents`.
///
/// Adds a new agent to an existing team. A conversation is
/// created automatically for the new agent.
#[derive(Debug, Deserialize)]
pub struct AddAgentRequest {
    pub name: String,
    pub role: String,
    pub backend: String,
    pub model: String,
    #[serde(default)]
    pub custom_agent_id: Option<String>,
    // Foundry: Phase 2 (roles + capability tiers)
    /// Optional specialization (`architect | implementer | reviewer | qa | researcher`).
    #[serde(default)]
    pub specialization: Option<String>,
    // Foundry: Phase 2 (roles + capability tiers)
    /// Optional capability tier (`fast | balanced | smart`). Overrides
    /// backend/model resolution unless an explicit model is supplied.
    #[serde(default)]
    pub tier: Option<String>,
}

/// Request body for `PATCH /api/teams/:id/agents/:slotId/name`.
#[derive(Debug, Deserialize)]
pub struct RenameAgentRequest {
    pub name: String,
}

// ---------------------------------------------------------------------------
// C. Message & session — Request DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /api/teams/:id/messages`.
///
/// Sends a user message to the team lead's mailbox, triggering a
/// wake cycle. `files` is optional and — when present — forwarded
/// to the underlying agent together with the wake payload.
#[derive(Debug, Deserialize)]
pub struct SendTeamMessageRequest {
    pub content: String,
    #[serde(default)]
    pub files: Option<Vec<String>>,
}

/// Request body for `POST /api/teams/:id/agents/:slotId/messages`.
///
/// Sends a user message directly to a specific agent's mailbox.
/// `files` semantics match [`SendTeamMessageRequest`].
#[derive(Debug, Deserialize)]
pub struct SendAgentMessageRequest {
    pub content: String,
    #[serde(default)]
    pub files: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// D. Team management — Response DTOs
// ---------------------------------------------------------------------------

/// Single agent within a team response.
///
/// Corresponds to the `TeamAgent` shared type in the API Spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamAgentResponse {
    pub slot_id: String,
    pub name: String,
    pub role: String,
    pub conversation_id: String,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default)]
    pub pending_confirmations: usize,
    // Foundry: Phase 2 (roles + capability tiers)
    /// Agent specialization (`lead | architect | implementer | reviewer | qa | researcher`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specialization: Option<String>,
    // Foundry: Phase 2 (roles + capability tiers)
    /// Capability tier the agent was staffed with (`fast | balanced | smart`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

/// Full team response returned by create, get, and list endpoints.
///
/// Corresponds to the `TTeam` shared type in the API Spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamResponse {
    pub id: String,
    pub name: String,
    pub agents: Vec<TeamAgentResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead_agent_id: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

/// Type alias for team list responses.
pub type TeamListResponse = Vec<TeamResponse>;

// ---------------------------------------------------------------------------
// E. WebSocket event payloads
// ---------------------------------------------------------------------------

/// Payload for `team.agent.status` WebSocket event.
///
/// Pushed when an agent's runtime status changes (e.g., idle → working).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamAgentStatusPayload {
    pub team_id: String,
    pub slot_id: String,
    pub status: String,
}

/// Payload for `team.agent.spawned` WebSocket event.
///
/// Pushed when the lead dynamically creates a new agent via
/// `team_spawn_agent`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamAgentSpawnedPayload {
    pub team_id: String,
    pub agent: TeamAgentResponse,
}

/// Payload for `team.agent.removed` WebSocket event.
///
/// Pushed when an agent is removed from the team.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamAgentRemovedPayload {
    pub team_id: String,
    pub slot_id: String,
}

/// Payload for `team.agent.renamed` WebSocket event.
///
/// Pushed when an agent's display name is changed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamAgentRenamedPayload {
    pub team_id: String,
    pub slot_id: String,
    pub name: String,
}

/// Payload for `team.agent.shutdown` WebSocket event.
///
/// Pushed when a teammate acknowledges a Lead-initiated shutdown by
/// replying `shutdown_approved`. The acknowledging teammate is identified
/// by `slot_id`; `remove_agent` (and the accompanying
/// `team.agent.removed` event) follows asynchronously once the agent
/// process is actually killed and state is cleared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamAgentShutdownPayload {
    pub team_id: String,
    pub slot_id: String,
}

/// Lifecycle phases of the per-team MCP stdio bridge + ACP session.
///
/// Emitted by the MCP supervisor whenever a teammate slot transitions
/// through its bring-up / degraded / ready states so the frontend can
/// surface actionable status for each agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TeamMcpPhase {
    TcpReady,
    TcpError,
    SessionInjecting,
    SessionReady,
    SessionError,
    LoadFailed,
    Degraded,
    ConfigWriteFailed,
    McpToolsWaiting,
    McpToolsReady,
}

/// Payload for `team.mcp.status` WebSocket event.
///
/// Pushed whenever a teammate's MCP bridge or ACP session transitions to
/// a new [`TeamMcpPhase`]. Optional fields carry phase-specific detail:
/// `port` for TCP bring-up, `server_count` for tool readiness, `error`
/// for failure phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMcpStatusPayload {
    pub team_id: String,
    pub slot_id: String,
    pub phase: TeamMcpPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Payload for `team.teammate.message` WebSocket event.
///
/// Pushed when a teammate sends a message to another agent within the
/// team; identifies both the sender (`from_slot_id` / `from_name`) and
/// the conversation the message belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeammateMessagePayload {
    pub conversation_id: String,
    pub content: String,
    pub from_slot_id: String,
    pub from_name: String,
}

// Foundry: Phase 1 (task/mailbox API)
// ---------------------------------------------------------------------------
// F. Task board — Request & Response DTOs
// ---------------------------------------------------------------------------

/// Foundry: Phase 1 (task/mailbox API)
/// Single task within a task board response. Mirrors the `TeamTask`
/// domain type (`aionui-team::types::TeamTask`) field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamTaskResponse {
    pub id: String,
    pub team_id: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub blocked_by: Vec<String>,
    pub blocks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Response body for `GET /api/teams/:id/tasks`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamTaskListResponse {
    pub tasks: Vec<TeamTaskResponse>,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Wrapper for single-task responses (`POST`/`PATCH` task endpoints),
/// so the task object lands under `data.task`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamTaskWrapper {
    pub task: TeamTaskResponse,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Request body for `POST /api/teams/:id/tasks`.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub subject: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub blocked_by: Option<Vec<String>>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Request body for `PATCH /api/teams/:id/tasks/:task_id`.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub blocked_by: Option<Vec<String>>,
    #[serde(default)]
    pub blocks: Option<Vec<String>>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

// Foundry: Phase 1 (task/mailbox API)
// ---------------------------------------------------------------------------
// G. Mailbox — Response DTOs
// ---------------------------------------------------------------------------

/// Foundry: Phase 1 (task/mailbox API)
/// Single mailbox message within a mailbox response. Mirrors the
/// `MailboxMessage` domain type (`aionui-team::types::MailboxMessage`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MailboxMessageResponse {
    pub id: String,
    pub team_id: String,
    pub to_agent_id: String,
    pub from_agent_id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    pub read: bool,
    pub created_at: TimestampMs,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Response body for `GET /api/teams/:id/mailbox`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MailboxListResponse {
    pub messages: Vec<MailboxMessageResponse>,
}

// Foundry: Phase 1 (task/mailbox API)
// ---------------------------------------------------------------------------
// H. Task board & mailbox — WebSocket event payloads
// ---------------------------------------------------------------------------

/// Foundry: Phase 1 (task/mailbox API)
/// Payload for `team.task.created` WebSocket event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamTaskCreatedPayload {
    pub team_id: String,
    pub task: TeamTaskResponse,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Payload for `team.task.updated` WebSocket event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamTaskUpdatedPayload {
    pub team_id: String,
    pub task: TeamTaskResponse,
}

/// Foundry: Phase 1 (task/mailbox API)
/// Payload for `team.mailbox.message` WebSocket event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamMailboxMessagePayload {
    pub team_id: String,
    pub message: MailboxMessageResponse,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- A. Team management requests ------------------------------------------

    #[test]
    fn deserialize_create_team_request_full() {
        let raw = json!({
            "name": "Team Alpha",
            "agents": [
                {
                    "name": "Lead",
                    "role": "lead",
                    "backend": "acp",
                    "model": "claude",
                    "custom_agent_id": "agent-x"
                },
                {
                    "name": "Worker",
                    "role": "teammate",
                    "backend": "acp",
                    "model": "claude"
                }
            ]
        });
        let req: CreateTeamRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name, "Team Alpha");
        assert_eq!(req.agents.len(), 2);
        assert_eq!(req.agents[0].name, "Lead");
        assert_eq!(req.agents[0].role, "lead");
        assert_eq!(req.agents[0].backend, "acp");
        assert_eq!(req.agents[0].model, "claude");
        assert_eq!(req.agents[0].custom_agent_id.as_deref(), Some("agent-x"));
        assert_eq!(req.agents[1].name, "Worker");
        assert!(req.agents[1].custom_agent_id.is_none());
    }

    #[test]
    fn deserialize_team_agent_input_with_conversation_id() {
        let raw = json!({
            "name": "Lead",
            "role": "lead",
            "backend": "acp",
            "model": "claude",
            "conversation_id": "existing-conv-123"
        });
        let input: TeamAgentInput = serde_json::from_value(raw).unwrap();
        assert_eq!(input.conversation_id.as_deref(), Some("existing-conv-123"));
    }

    #[test]
    fn deserialize_team_agent_input_conversation_id_defaults_to_none() {
        let raw = json!({
            "name": "Lead",
            "role": "lead",
            "backend": "acp",
            "model": "claude"
        });
        let input: TeamAgentInput = serde_json::from_value(raw).unwrap();
        assert!(input.conversation_id.is_none());
    }

    #[test]
    fn deserialize_create_team_request_empty_agents() {
        let raw = json!({ "name": "Empty", "agents": [] });
        let req: CreateTeamRequest = serde_json::from_value(raw).unwrap();
        assert!(req.agents.is_empty());
    }

    #[test]
    fn deserialize_create_team_request_missing_name() {
        let raw = json!({ "agents": [] });
        let result = serde_json::from_value::<CreateTeamRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_create_team_request_missing_agents() {
        let raw = json!({ "name": "Team" });
        let result = serde_json::from_value::<CreateTeamRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_rename_team_request() {
        let raw = json!({ "name": "New Name" });
        let req: RenameTeamRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name, "New Name");
    }

    #[test]
    fn deserialize_rename_team_request_missing_name() {
        let raw = json!({});
        let result = serde_json::from_value::<RenameTeamRequest>(raw);
        assert!(result.is_err());
    }

    // -- B. Agent management requests -----------------------------------------

    #[test]
    fn deserialize_add_agent_request() {
        let raw = json!({
            "name": "Helper",
            "role": "teammate",
            "backend": "acp",
            "model": "claude"
        });
        let req: AddAgentRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name, "Helper");
        assert_eq!(req.role, "teammate");
        assert_eq!(req.backend, "acp");
        assert_eq!(req.model, "claude");
        assert!(req.custom_agent_id.is_none());
    }

    #[test]
    fn deserialize_add_agent_request_with_custom_agent_id() {
        let raw = json!({
            "name": "Custom",
            "role": "teammate",
            "backend": "acp",
            "model": "claude",
            "custom_agent_id": "custom-1"
        });
        let req: AddAgentRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.custom_agent_id.as_deref(), Some("custom-1"));
    }

    // Foundry: Phase 2 (roles + capability tiers)
    #[test]
    fn deserialize_add_agent_request_with_specialization_and_tier() {
        let raw = json!({
            "name": "Reviewer",
            "role": "teammate",
            "backend": "claude",
            "model": "",
            "specialization": "reviewer",
            "tier": "smart"
        });
        let req: AddAgentRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.specialization.as_deref(), Some("reviewer"));
        assert_eq!(req.tier.as_deref(), Some("smart"));
    }

    // Foundry: Phase 2 (roles + capability tiers)
    #[test]
    fn deserialize_add_agent_request_specialization_tier_default_to_none() {
        let raw = json!({ "name": "X", "role": "teammate", "backend": "acp", "model": "claude" });
        let req: AddAgentRequest = serde_json::from_value(raw).unwrap();
        assert!(req.specialization.is_none());
        assert!(req.tier.is_none());
    }

    #[test]
    fn deserialize_add_agent_request_missing_name() {
        let raw = json!({ "role": "teammate", "backend": "acp", "model": "claude" });
        let result = serde_json::from_value::<AddAgentRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_add_agent_request_missing_backend() {
        let raw = json!({ "name": "X", "role": "teammate", "model": "claude" });
        let result = serde_json::from_value::<AddAgentRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_rename_agent_request() {
        let raw = json!({ "name": "New Agent Name" });
        let req: RenameAgentRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name, "New Agent Name");
    }

    #[test]
    fn deserialize_rename_agent_request_missing_name() {
        let raw = json!({});
        let result = serde_json::from_value::<RenameAgentRequest>(raw);
        assert!(result.is_err());
    }

    // -- C. Message & session requests ----------------------------------------

    #[test]
    fn deserialize_send_team_message_request() {
        let raw = json!({ "content": "Hello team!" });
        let req: SendTeamMessageRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.content, "Hello team!");
    }

    #[test]
    fn deserialize_send_team_message_request_missing_content() {
        let raw = json!({});
        let result = serde_json::from_value::<SendTeamMessageRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_send_agent_message_request() {
        let raw = json!({ "content": "Do this task" });
        let req: SendAgentMessageRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.content, "Do this task");
    }

    #[test]
    fn deserialize_send_agent_message_request_missing_content() {
        let raw = json!({});
        let result = serde_json::from_value::<SendAgentMessageRequest>(raw);
        assert!(result.is_err());
    }

    // -- D. Response DTOs -----------------------------------------------------

    #[test]
    fn serialize_team_agent_response_snake_case() {
        let agent = TeamAgentResponse {
            slot_id: "slot-1".into(),
            name: "Lead Agent".into(),
            role: "lead".into(),
            conversation_id: "conv-1".into(),
            backend: "acp".into(),
            icon: Some("/api/assets/logos/ai-major/claude.svg".into()),
            model: "claude".into(),
            custom_agent_id: Some("agent-x".into()),
            status: Some("idle".into()),
            pending_confirmations: 2,
            specialization: None,
            tier: None,
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert_eq!(json["slot_id"], "slot-1");
        assert_eq!(json["name"], "Lead Agent");
        assert_eq!(json["role"], "lead");
        assert_eq!(json["conversation_id"], "conv-1");
        assert_eq!(json["backend"], "acp");
        assert_eq!(json["icon"], "/api/assets/logos/ai-major/claude.svg");
        assert_eq!(json["model"], "claude");
        assert_eq!(json["custom_agent_id"], "agent-x");
        assert_eq!(json["status"], "idle");
        assert_eq!(json["pending_confirmations"], 2);
    }

    #[test]
    fn serialize_team_agent_response_optional_fields_omitted() {
        let agent = TeamAgentResponse {
            slot_id: "slot-2".into(),
            name: "Worker".into(),
            role: "teammate".into(),
            conversation_id: "conv-2".into(),
            backend: "acp".into(),
            icon: None,
            model: "claude".into(),
            custom_agent_id: None,
            status: None,
            pending_confirmations: 0,
            specialization: None,
            tier: None,
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert!(json.get("icon").is_none());
        assert!(json.get("custom_agent_id").is_none());
        assert!(json.get("status").is_none());
    }

    #[test]
    fn serialize_team_response_snake_case() {
        let team = TeamResponse {
            id: "team-1".into(),
            name: "Alpha".into(),
            agents: vec![TeamAgentResponse {
                slot_id: "slot-1".into(),
                name: "Lead".into(),
                role: "lead".into(),
                conversation_id: "conv-1".into(),
                backend: "acp".into(),
                icon: Some("/api/assets/logos/ai-major/claude.svg".into()),
                model: "claude".into(),
                custom_agent_id: None,
                status: None,
                pending_confirmations: 0,
                specialization: None,
                tier: None,
            }],
            lead_agent_id: Some("slot-1".into()),
            created_at: 1700000000000,
            updated_at: 1700001000000,
        };
        let json = serde_json::to_value(&team).unwrap();
        assert_eq!(json["id"], "team-1");
        assert_eq!(json["name"], "Alpha");
        assert_eq!(json["lead_agent_id"], "slot-1");
        assert_eq!(json["created_at"], 1700000000000_i64);
        assert_eq!(json["updated_at"], 1700001000000_i64);
        assert_eq!(json["agents"].as_array().unwrap().len(), 1);
        assert_eq!(json["agents"][0]["slot_id"], "slot-1");
    }

    #[test]
    fn serialize_team_response_no_lead() {
        let team = TeamResponse {
            id: "team-2".into(),
            name: "Beta".into(),
            agents: vec![],
            lead_agent_id: None,
            created_at: 1700000000000,
            updated_at: 1700000000000,
        };
        let json = serde_json::to_value(&team).unwrap();
        assert!(json.get("lead_agent_id").is_none());
        assert!(json["agents"].as_array().unwrap().is_empty());
    }

    // -- E. WebSocket event payloads ------------------------------------------

    #[test]
    fn serialize_team_agent_status_payload() {
        let payload = TeamAgentStatusPayload {
            team_id: "team-1".into(),
            slot_id: "slot-1".into(),
            status: "working".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["slot_id"], "slot-1");
        assert_eq!(json["status"], "working");
    }

    #[test]
    fn serialize_team_agent_spawned_payload() {
        let payload = TeamAgentSpawnedPayload {
            team_id: "team-1".into(),
            agent: TeamAgentResponse {
                slot_id: "slot-3".into(),
                name: "Dynamic Worker".into(),
                role: "teammate".into(),
                conversation_id: "conv-3".into(),
                backend: "claude".into(),
                icon: Some("/api/assets/logos/ai-major/claude.svg".into()),
                model: "opus".into(),
                custom_agent_id: None,
                status: Some("idle".into()),
                pending_confirmations: 0,
                specialization: None,
                tier: None,
            },
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["agent"]["slot_id"], "slot-3");
        assert_eq!(json["agent"]["name"], "Dynamic Worker");
        assert_eq!(json["agent"]["role"], "teammate");
        assert_eq!(json["agent"]["status"], "idle");
    }

    #[test]
    fn serialize_team_agent_removed_payload() {
        let payload = TeamAgentRemovedPayload {
            team_id: "team-1".into(),
            slot_id: "slot-2".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["slot_id"], "slot-2");
    }

    #[test]
    fn serialize_team_agent_renamed_payload() {
        let payload = TeamAgentRenamedPayload {
            team_id: "team-1".into(),
            slot_id: "slot-1".into(),
            name: "Renamed Agent".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["slot_id"], "slot-1");
        assert_eq!(json["name"], "Renamed Agent");
    }

    // -- Roundtrip tests ------------------------------------------------------

    #[test]
    fn team_agent_response_roundtrip() {
        let agent = TeamAgentResponse {
            slot_id: "slot-1".into(),
            name: "Agent".into(),
            role: "lead".into(),
            conversation_id: "conv-1".into(),
            backend: "acp".into(),
            icon: Some("/api/assets/logos/ai-major/claude.svg".into()),
            model: "claude".into(),
            custom_agent_id: Some("custom-1".into()),
            status: Some("working".into()),
            pending_confirmations: 1,
            specialization: None,
            tier: None,
        };
        let json = serde_json::to_string(&agent).unwrap();
        let parsed: TeamAgentResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, agent);
    }

    // Foundry: Phase 2 (roles + capability tiers)
    #[test]
    fn team_agent_response_specialization_tier_roundtrip() {
        let agent = TeamAgentResponse {
            slot_id: "slot-1".into(),
            name: "Architect".into(),
            role: "teammate".into(),
            conversation_id: "conv-1".into(),
            backend: "claude".into(),
            icon: None,
            model: "claude-opus".into(),
            custom_agent_id: None,
            status: Some("idle".into()),
            pending_confirmations: 0,
            specialization: Some("architect".into()),
            tier: Some("smart".into()),
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert_eq!(json["specialization"], "architect");
        assert_eq!(json["tier"], "smart");
        let parsed: TeamAgentResponse = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, agent);
    }

    // Foundry: Phase 2 (roles + capability tiers)
    #[test]
    fn team_agent_response_specialization_tier_omitted_when_none() {
        let agent = TeamAgentResponse {
            slot_id: "slot-1".into(),
            name: "Worker".into(),
            role: "teammate".into(),
            conversation_id: "conv-1".into(),
            backend: "claude".into(),
            icon: None,
            model: "claude".into(),
            custom_agent_id: None,
            status: None,
            pending_confirmations: 0,
            specialization: None,
            tier: None,
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert!(json.get("specialization").is_none());
        assert!(json.get("tier").is_none());
    }

    #[test]
    fn team_response_roundtrip() {
        let team = TeamResponse {
            id: "team-1".into(),
            name: "Alpha".into(),
            agents: vec![
                TeamAgentResponse {
                    slot_id: "s1".into(),
                    name: "Lead".into(),
                    role: "lead".into(),
                    conversation_id: "c1".into(),
                    backend: "acp".into(),
                    icon: None,
                    model: "claude".into(),
                    custom_agent_id: None,
                    status: None,
                    pending_confirmations: 0,
                    specialization: None,
                    tier: None,
                },
                TeamAgentResponse {
                    slot_id: "s2".into(),
                    name: "Worker".into(),
                    role: "teammate".into(),
                    conversation_id: "c2".into(),
                    backend: "acp".into(),
                    icon: Some("/api/assets/logos/tools/coding/codex.svg".into()),
                    model: "claude".into(),
                    custom_agent_id: Some("x".into()),
                    status: Some("idle".into()),
                    pending_confirmations: 3,
                    specialization: None,
                    tier: None,
                },
            ],
            lead_agent_id: Some("s1".into()),
            created_at: 1000,
            updated_at: 2000,
        };
        let json = serde_json::to_string(&team).unwrap();
        let parsed: TeamResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, team);
    }

    #[test]
    fn team_agent_status_payload_roundtrip() {
        let payload = TeamAgentStatusPayload {
            team_id: "t1".into(),
            slot_id: "s1".into(),
            status: "thinking".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: TeamAgentStatusPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn team_agent_spawned_payload_roundtrip() {
        let payload = TeamAgentSpawnedPayload {
            team_id: "t1".into(),
            agent: TeamAgentResponse {
                slot_id: "s3".into(),
                name: "New".into(),
                role: "teammate".into(),
                conversation_id: "c3".into(),
                backend: "claude".into(),
                icon: None,
                model: "sonnet".into(),
                custom_agent_id: None,
                status: None,
                pending_confirmations: 0,
                specialization: None,
                tier: None,
            },
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: TeamAgentSpawnedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn team_agent_removed_payload_roundtrip() {
        let payload = TeamAgentRemovedPayload {
            team_id: "t1".into(),
            slot_id: "s2".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: TeamAgentRemovedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn team_agent_renamed_payload_roundtrip() {
        let payload = TeamAgentRenamedPayload {
            team_id: "t1".into(),
            slot_id: "s1".into(),
            name: "Renamed".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: TeamAgentRenamedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn team_agent_shutdown_payload_roundtrip() {
        let payload = TeamAgentShutdownPayload {
            team_id: "t1".into(),
            slot_id: "s2".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "t1");
        assert_eq!(json["slot_id"], "s2");
        let parsed: TeamAgentShutdownPayload = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, payload);
    }

    // -- Deserialize from snake_case JSON (matching Rust field names) -----------

    #[test]
    fn deserialize_team_agent_response_from_snake_case() {
        let raw = json!({
            "slot_id": "s1",
            "name": "Agent",
            "role": "lead",
            "conversation_id": "c1",
            "backend": "acp",
            "model": "claude",
            "custom_agent_id": "cust-1",
            "status": "idle"
        });
        let agent: TeamAgentResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(agent.slot_id, "s1");
        assert_eq!(agent.conversation_id, "c1");
        assert_eq!(agent.custom_agent_id.as_deref(), Some("cust-1"));
        assert_eq!(agent.status.as_deref(), Some("idle"));
        assert_eq!(agent.pending_confirmations, 0);
    }

    #[test]
    fn deserialize_team_response_from_snake_case() {
        let raw = json!({
            "id": "team-1",
            "name": "Alpha",
            "agents": [],
            "lead_agent_id": "s1",
            "created_at": 1000,
            "updated_at": 2000
        });
        let team: TeamResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(team.id, "team-1");
        assert_eq!(team.lead_agent_id.as_deref(), Some("s1"));
        assert_eq!(team.created_at, 1000);
    }

    // -- F. TeamMcpPhase serde roundtrip --------------------------------------

    fn assert_phase_roundtrip(phase: TeamMcpPhase, wire: &str) {
        let json = serde_json::to_value(&phase).unwrap();
        assert_eq!(json, serde_json::Value::String(wire.into()));
        let parsed: TeamMcpPhase = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, phase);
    }

    #[test]
    fn team_mcp_phase_tcp_ready_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::TcpReady, "tcp_ready");
    }

    #[test]
    fn team_mcp_phase_tcp_error_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::TcpError, "tcp_error");
    }

    #[test]
    fn team_mcp_phase_session_injecting_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::SessionInjecting, "session_injecting");
    }

    #[test]
    fn team_mcp_phase_session_ready_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::SessionReady, "session_ready");
    }

    #[test]
    fn team_mcp_phase_session_error_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::SessionError, "session_error");
    }

    #[test]
    fn team_mcp_phase_load_failed_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::LoadFailed, "load_failed");
    }

    #[test]
    fn team_mcp_phase_degraded_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::Degraded, "degraded");
    }

    #[test]
    fn team_mcp_phase_config_write_failed_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::ConfigWriteFailed, "config_write_failed");
    }

    #[test]
    fn team_mcp_phase_mcp_tools_waiting_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::McpToolsWaiting, "mcp_tools_waiting");
    }

    #[test]
    fn team_mcp_phase_mcp_tools_ready_roundtrip() {
        assert_phase_roundtrip(TeamMcpPhase::McpToolsReady, "mcp_tools_ready");
    }

    // -- G. TeamMcpStatusPayload & TeammateMessagePayload ---------------------

    #[test]
    fn serialize_team_mcp_status_payload_all_fields_present() {
        let payload = TeamMcpStatusPayload {
            team_id: "team-1".into(),
            slot_id: "slot-2".into(),
            phase: TeamMcpPhase::SessionReady,
            port: Some(54321),
            server_count: Some(7),
            error: Some("boom".into()),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["slot_id"], "slot-2");
        assert_eq!(json["phase"], "session_ready");
        assert_eq!(json["port"], 54321);
        assert_eq!(json["server_count"], 7);
        assert_eq!(json["error"], "boom");
    }

    #[test]
    fn serialize_team_mcp_status_payload_optional_fields_omitted() {
        let payload = TeamMcpStatusPayload {
            team_id: "team-1".into(),
            slot_id: "slot-2".into(),
            phase: TeamMcpPhase::TcpReady,
            port: None,
            server_count: None,
            error: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["slot_id"], "slot-2");
        assert_eq!(json["phase"], "tcp_ready");
        assert!(json.get("port").is_none());
        assert!(json.get("server_count").is_none());
        assert!(json.get("error").is_none());
    }

    #[test]
    fn serialize_teammate_message_payload_all_fields_present() {
        let payload = TeammateMessagePayload {
            conversation_id: "conv-9".into(),
            content: "ping".into(),
            from_slot_id: "slot-1".into(),
            from_name: "Lead".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["conversation_id"], "conv-9");
        assert_eq!(json["content"], "ping");
        assert_eq!(json["from_slot_id"], "slot-1");
        assert_eq!(json["from_name"], "Lead");
    }

    // Foundry: Phase 1 (task/mailbox API)
    // -- H. Task board DTOs ---------------------------------------------------

    fn sample_task_response() -> TeamTaskResponse {
        TeamTaskResponse {
            id: "tk-1".into(),
            team_id: "team-1".into(),
            subject: "Implement feature".into(),
            description: Some("Details".into()),
            status: "pending".into(),
            owner: Some("slot-1".into()),
            blocked_by: vec!["tk-0".into()],
            blocks: vec!["tk-2".into()],
            metadata: Some(json!({ "priority": "high" })),
            created_at: 1000,
            updated_at: 2000,
        }
    }

    #[test]
    fn serialize_team_task_response_snake_case() {
        let json = serde_json::to_value(sample_task_response()).unwrap();
        assert_eq!(json["id"], "tk-1");
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["subject"], "Implement feature");
        assert_eq!(json["status"], "pending");
        assert_eq!(json["owner"], "slot-1");
        assert_eq!(json["blocked_by"][0], "tk-0");
        assert_eq!(json["blocks"][0], "tk-2");
        assert_eq!(json["metadata"]["priority"], "high");
        assert_eq!(json["created_at"], 1000_i64);
        assert_eq!(json["updated_at"], 2000_i64);
    }

    #[test]
    fn serialize_team_task_response_optional_fields_omitted() {
        let task = TeamTaskResponse {
            id: "tk-1".into(),
            team_id: "team-1".into(),
            subject: "Simple".into(),
            description: None,
            status: "pending".into(),
            owner: None,
            blocked_by: vec![],
            blocks: vec![],
            metadata: None,
            created_at: 0,
            updated_at: 0,
        };
        let json = serde_json::to_value(&task).unwrap();
        assert!(json.get("description").is_none());
        assert!(json.get("owner").is_none());
        assert!(json.get("metadata").is_none());
        assert!(json["blocked_by"].as_array().unwrap().is_empty());
    }

    #[test]
    fn team_task_response_roundtrip() {
        let task = sample_task_response();
        let json = serde_json::to_string(&task).unwrap();
        let parsed: TeamTaskResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, task);
    }

    #[test]
    fn serialize_team_task_list_response() {
        let list = TeamTaskListResponse {
            tasks: vec![sample_task_response()],
        };
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(json["tasks"].as_array().unwrap().len(), 1);
        assert_eq!(json["tasks"][0]["id"], "tk-1");
    }

    #[test]
    fn serialize_team_task_wrapper() {
        let wrapper = TeamTaskWrapper {
            task: sample_task_response(),
        };
        let json = serde_json::to_value(&wrapper).unwrap();
        assert_eq!(json["task"]["id"], "tk-1");
    }

    #[test]
    fn deserialize_create_task_request_full() {
        let raw = json!({
            "subject": "Build it",
            "description": "Do the thing",
            "owner": "slot-1",
            "blocked_by": ["tk-0"],
            "metadata": { "priority": "high" }
        });
        let req: CreateTaskRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.subject, "Build it");
        assert_eq!(req.description.as_deref(), Some("Do the thing"));
        assert_eq!(req.owner.as_deref(), Some("slot-1"));
        assert_eq!(req.blocked_by.unwrap(), vec!["tk-0"]);
        assert!(req.metadata.is_some());
    }

    #[test]
    fn deserialize_create_task_request_minimal() {
        let raw = json!({ "subject": "Just a subject" });
        let req: CreateTaskRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.subject, "Just a subject");
        assert!(req.description.is_none());
        assert!(req.owner.is_none());
        assert!(req.blocked_by.is_none());
        assert!(req.metadata.is_none());
    }

    #[test]
    fn deserialize_create_task_request_missing_subject() {
        let raw = json!({ "description": "no subject" });
        let result = serde_json::from_value::<CreateTaskRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_update_task_request_full() {
        let raw = json!({
            "subject": "New subject",
            "description": "New desc",
            "status": "in_progress",
            "owner": "slot-2",
            "blocked_by": ["tk-1"],
            "blocks": ["tk-3"],
            "metadata": { "k": "v" }
        });
        let req: UpdateTaskRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.subject.as_deref(), Some("New subject"));
        assert_eq!(req.status.as_deref(), Some("in_progress"));
        assert_eq!(req.owner.as_deref(), Some("slot-2"));
        assert_eq!(req.blocked_by.unwrap(), vec!["tk-1"]);
        assert_eq!(req.blocks.unwrap(), vec!["tk-3"]);
    }

    #[test]
    fn deserialize_update_task_request_empty() {
        let raw = json!({});
        let req: UpdateTaskRequest = serde_json::from_value(raw).unwrap();
        assert!(req.subject.is_none());
        assert!(req.status.is_none());
        assert!(req.metadata.is_none());
    }

    // Foundry: Phase 1 (task/mailbox API)
    // -- I. Mailbox DTOs ------------------------------------------------------

    fn sample_mailbox_response() -> MailboxMessageResponse {
        MailboxMessageResponse {
            id: "m-1".into(),
            team_id: "team-1".into(),
            to_agent_id: "slot-1".into(),
            from_agent_id: "slot-2".into(),
            msg_type: "message".into(),
            content: "hello".into(),
            summary: Some("greeting".into()),
            files: Some(vec!["/tmp/a.txt".into()]),
            read: false,
            created_at: 1000,
        }
    }

    #[test]
    fn serialize_mailbox_message_response_type_field() {
        let json = serde_json::to_value(sample_mailbox_response()).unwrap();
        assert_eq!(json["id"], "m-1");
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["to_agent_id"], "slot-1");
        assert_eq!(json["from_agent_id"], "slot-2");
        assert_eq!(json["type"], "message");
        assert!(json.get("msg_type").is_none(), "must serialize as 'type'");
        assert_eq!(json["content"], "hello");
        assert_eq!(json["summary"], "greeting");
        assert_eq!(json["files"][0], "/tmp/a.txt");
        assert_eq!(json["read"], false);
        assert_eq!(json["created_at"], 1000_i64);
    }

    #[test]
    fn serialize_mailbox_message_response_optional_fields_omitted() {
        let msg = MailboxMessageResponse {
            id: "m-2".into(),
            team_id: "team-1".into(),
            to_agent_id: "slot-1".into(),
            from_agent_id: "user".into(),
            msg_type: "message".into(),
            content: "x".into(),
            summary: None,
            files: None,
            read: true,
            created_at: 0,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.get("summary").is_none());
        assert!(json.get("files").is_none());
    }

    #[test]
    fn mailbox_message_response_roundtrip() {
        let msg = sample_mailbox_response();
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: MailboxMessageResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn serialize_mailbox_list_response() {
        let list = MailboxListResponse {
            messages: vec![sample_mailbox_response()],
        };
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(json["messages"].as_array().unwrap().len(), 1);
        assert_eq!(json["messages"][0]["id"], "m-1");
    }

    // Foundry: Phase 1 (task/mailbox API)
    // -- J. Task board & mailbox WebSocket payloads ---------------------------

    #[test]
    fn serialize_team_task_created_payload() {
        let payload = TeamTaskCreatedPayload {
            team_id: "team-1".into(),
            task: sample_task_response(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["task"]["id"], "tk-1");
    }

    #[test]
    fn team_task_created_payload_roundtrip() {
        let payload = TeamTaskCreatedPayload {
            team_id: "team-1".into(),
            task: sample_task_response(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: TeamTaskCreatedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn serialize_team_task_updated_payload() {
        let payload = TeamTaskUpdatedPayload {
            team_id: "team-1".into(),
            task: sample_task_response(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["task"]["id"], "tk-1");
    }

    #[test]
    fn serialize_team_mailbox_message_payload() {
        let payload = TeamMailboxMessagePayload {
            team_id: "team-1".into(),
            message: sample_mailbox_response(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["team_id"], "team-1");
        assert_eq!(json["message"]["id"], "m-1");
        assert_eq!(json["message"]["type"], "message");
    }

    #[test]
    fn team_mailbox_message_payload_roundtrip() {
        let payload = TeamMailboxMessagePayload {
            team_id: "team-1".into(),
            message: sample_mailbox_response(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: TeamMailboxMessagePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }
}

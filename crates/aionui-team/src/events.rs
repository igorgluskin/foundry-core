use std::sync::Arc;

use aionui_api_types::{
    TeamAgentRemovedPayload, TeamAgentRenamedPayload, TeamAgentShutdownPayload, TeamAgentSpawnedPayload,
    TeamAgentStatusPayload, WebSocketMessage,
};
// Foundry: Phase 1 (task/mailbox API)
use aionui_api_types::{TeamMailboxMessagePayload, TeamTaskCreatedPayload, TeamTaskUpdatedPayload};
use aionui_realtime::EventBroadcaster;

// Foundry: Phase 1 (task/mailbox API) — added MailboxMessage, TeamTask
use crate::types::{MailboxMessage, TeamAgent, TeamTask, TeammateStatus};

pub struct TeamEventEmitter {
    team_id: String,
    broadcaster: Arc<dyn EventBroadcaster>,
}

impl TeamEventEmitter {
    pub fn new(team_id: String, broadcaster: Arc<dyn EventBroadcaster>) -> Self {
        Self { team_id, broadcaster }
    }

    pub fn team_id(&self) -> &str {
        &self.team_id
    }

    pub fn broadcast_agent_status(&self, slot_id: &str, status: TeammateStatus) {
        let payload = TeamAgentStatusPayload {
            team_id: self.team_id.clone(),
            slot_id: slot_id.to_owned(),
            status: status.to_string(),
        };
        let event = WebSocketMessage::new(
            "team.agent.status",
            serde_json::to_value(payload).expect("serialize status payload"),
        );
        self.broadcaster.broadcast(event);
    }

    pub fn broadcast_agent_spawned(&self, agent: &TeamAgent) {
        let payload = TeamAgentSpawnedPayload {
            team_id: self.team_id.clone(),
            agent: agent.to_response(),
        };
        let event = WebSocketMessage::new(
            "team.agent.spawned",
            serde_json::to_value(payload).expect("serialize spawned payload"),
        );
        self.broadcaster.broadcast(event);
    }

    pub fn broadcast_agent_removed(&self, slot_id: &str) {
        let payload = TeamAgentRemovedPayload {
            team_id: self.team_id.clone(),
            slot_id: slot_id.to_owned(),
        };
        let event = WebSocketMessage::new(
            "team.agent.removed",
            serde_json::to_value(payload).expect("serialize removed payload"),
        );
        self.broadcaster.broadcast(event);
    }

    /// Emit `team.agent.shutdown` to signal that the named teammate has
    /// acknowledged a Lead-initiated shutdown request. The actual removal
    /// (and `team.agent.removed`) follows once the agent process is killed
    /// and scheduler state is cleared.
    pub fn broadcast_agent_shutdown(&self, slot_id: &str) {
        let payload = TeamAgentShutdownPayload {
            team_id: self.team_id.clone(),
            slot_id: slot_id.to_owned(),
        };
        let event = WebSocketMessage::new(
            "team.agent.shutdown",
            serde_json::to_value(payload).expect("serialize shutdown payload"),
        );
        self.broadcaster.broadcast(event);
    }

    pub fn broadcast_agent_renamed(&self, slot_id: &str, name: &str) {
        let payload = TeamAgentRenamedPayload {
            team_id: self.team_id.clone(),
            slot_id: slot_id.to_owned(),
            name: name.to_owned(),
        };
        let event = WebSocketMessage::new(
            "team.agent.renamed",
            serde_json::to_value(payload).expect("serialize renamed payload"),
        );
        self.broadcaster.broadcast(event);
    }

    // Foundry: Phase 1 (task/mailbox API)
    /// Emit `team.task.created` after a task is added to the board.
    pub fn broadcast_task_created(&self, task: &TeamTask) {
        let payload = TeamTaskCreatedPayload {
            team_id: self.team_id.clone(),
            task: task.to_response(),
        };
        let event = WebSocketMessage::new(
            "team.task.created",
            serde_json::to_value(payload).expect("serialize task created payload"),
        );
        self.broadcaster.broadcast(event);
    }

    // Foundry: Phase 1 (task/mailbox API)
    /// Emit `team.task.updated` after a task's fields change.
    pub fn broadcast_task_updated(&self, task: &TeamTask) {
        let payload = TeamTaskUpdatedPayload {
            team_id: self.team_id.clone(),
            task: task.to_response(),
        };
        let event = WebSocketMessage::new(
            "team.task.updated",
            serde_json::to_value(payload).expect("serialize task updated payload"),
        );
        self.broadcaster.broadcast(event);
    }

    // Foundry: Phase 1 (task/mailbox API)
    /// Emit `team.mailbox.message` after a message is written to the mailbox.
    pub fn broadcast_mailbox_message(&self, message: &MailboxMessage) {
        let payload = TeamMailboxMessagePayload {
            team_id: self.team_id.clone(),
            message: message.to_response(),
        };
        let event = WebSocketMessage::new(
            "team.mailbox.message",
            serde_json::to_value(payload).expect("serialize mailbox message payload"),
        );
        self.broadcaster.broadcast(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TeammateRole;
    use aionui_api_types::{
        TeamAgentRemovedPayload, TeamAgentRenamedPayload, TeamAgentShutdownPayload, TeamAgentSpawnedPayload,
        TeamAgentStatusPayload,
    };
    // Foundry: Phase 1 (task/mailbox API)
    use crate::types::{MailboxMessageType, TaskStatus};
    use aionui_api_types::{TeamMailboxMessagePayload, TeamTaskCreatedPayload, TeamTaskUpdatedPayload};

    struct RecordingBroadcaster {
        events: std::sync::Mutex<Vec<WebSocketMessage<serde_json::Value>>>,
    }

    impl RecordingBroadcaster {
        fn new() -> Self {
            Self {
                events: std::sync::Mutex::new(vec![]),
            }
        }

        fn events(&self) -> Vec<WebSocketMessage<serde_json::Value>> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventBroadcaster for RecordingBroadcaster {
        fn broadcast(&self, event: WebSocketMessage<serde_json::Value>) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn make_emitter() -> (TeamEventEmitter, Arc<RecordingBroadcaster>) {
        let bc = Arc::new(RecordingBroadcaster::new());
        let emitter = TeamEventEmitter::new("team-1".into(), bc.clone());
        (emitter, bc)
    }

    #[test]
    fn status_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_agent_status("slot-1", TeammateStatus::Working);

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.agent.status");

        let payload: TeamAgentStatusPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.slot_id, "slot-1");
        assert_eq!(payload.status, "working");
    }

    #[test]
    fn spawned_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        let agent = TeamAgent {
            slot_id: "slot-2".into(),
            name: "Worker".into(),
            role: TeammateRole::Teammate,
            conversation_id: "conv-2".into(),
            backend: "acp".into(),
            model: "claude".into(),
            custom_agent_id: None,
            status: Some(TeammateStatus::Idle),
            conversation_type: None,
            cli_path: None,
        };
        emitter.broadcast_agent_spawned(&agent);

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.agent.spawned");

        let payload: TeamAgentSpawnedPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.agent.slot_id, "slot-2");
        assert_eq!(payload.agent.name, "Worker");
        assert_eq!(payload.agent.role, "teammate");
    }

    #[test]
    fn removed_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_agent_removed("slot-3");

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.agent.removed");

        let payload: TeamAgentRemovedPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.slot_id, "slot-3");
    }

    #[test]
    fn shutdown_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_agent_shutdown("slot-9");

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.agent.shutdown");

        let payload: TeamAgentShutdownPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.slot_id, "slot-9");
    }

    #[test]
    fn renamed_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_agent_renamed("slot-1", "New Name");

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.agent.renamed");

        let payload: TeamAgentRenamedPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.slot_id, "slot-1");
        assert_eq!(payload.name, "New Name");
    }

    // Foundry: Phase 1 (task/mailbox API)
    fn make_task() -> TeamTask {
        TeamTask {
            id: "tk-1".into(),
            team_id: "team-1".into(),
            subject: "Implement feature".into(),
            description: Some("Details".into()),
            status: TaskStatus::Pending,
            owner: Some("slot-1".into()),
            blocked_by: vec!["tk-0".into()],
            blocks: vec!["tk-2".into()],
            metadata: Some(serde_json::json!({ "priority": "high" })),
            created_at: 1000,
            updated_at: 2000,
        }
    }

    // Foundry: Phase 1 (task/mailbox API)
    fn make_message() -> MailboxMessage {
        MailboxMessage {
            id: "m-1".into(),
            team_id: "team-1".into(),
            to_agent_id: "slot-1".into(),
            from_agent_id: "slot-2".into(),
            msg_type: MailboxMessageType::Message,
            content: "hello".into(),
            summary: None,
            files: None,
            read: false,
            created_at: 1000,
        }
    }

    // Foundry: Phase 1 (task/mailbox API)
    #[test]
    fn task_created_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_task_created(&make_task());

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.task.created");

        let payload: TeamTaskCreatedPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.task.id, "tk-1");
        assert_eq!(payload.task.subject, "Implement feature");
        assert_eq!(payload.task.status, "pending");
    }

    // Foundry: Phase 1 (task/mailbox API)
    #[test]
    fn task_updated_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        let mut task = make_task();
        task.status = TaskStatus::Completed;
        emitter.broadcast_task_updated(&task);

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.task.updated");

        let payload: TeamTaskUpdatedPayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.task.id, "tk-1");
        assert_eq!(payload.task.status, "completed");
    }

    // Foundry: Phase 1 (task/mailbox API)
    #[test]
    fn mailbox_message_event_has_correct_shape() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_mailbox_message(&make_message());

        let events = bc.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "team.mailbox.message");

        let payload: TeamMailboxMessagePayload = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(payload.team_id, "team-1");
        assert_eq!(payload.message.id, "m-1");
        assert_eq!(payload.message.msg_type, "message");
        assert_eq!(payload.message.content, "hello");
    }

    #[test]
    fn team_id_accessor() {
        let (emitter, _) = make_emitter();
        assert_eq!(emitter.team_id(), "team-1");
    }

    #[test]
    fn multiple_events_accumulate() {
        let (emitter, bc) = make_emitter();
        emitter.broadcast_agent_status("s1", TeammateStatus::Working);
        emitter.broadcast_agent_status("s1", TeammateStatus::Idle);
        emitter.broadcast_agent_removed("s2");

        let events = bc.events();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].name, "team.agent.status");
        assert_eq!(events[1].name, "team.agent.status");
        assert_eq!(events[2].name, "team.agent.removed");
    }

    #[test]
    fn all_status_variants_serialize() {
        let (emitter, bc) = make_emitter();
        let statuses = [
            TeammateStatus::Idle,
            TeammateStatus::Working,
            TeammateStatus::Thinking,
            TeammateStatus::ToolUse,
            TeammateStatus::Completed,
            TeammateStatus::Error,
        ];
        for s in statuses {
            emitter.broadcast_agent_status("s1", s);
        }

        let events = bc.events();
        assert_eq!(events.len(), 6);
        let expected = ["idle", "working", "thinking", "tool_use", "completed", "error"];
        for (event, exp) in events.iter().zip(expected.iter()) {
            let payload: TeamAgentStatusPayload = serde_json::from_value(event.data.clone()).unwrap();
            assert_eq!(payload.status, *exp);
        }
    }
}

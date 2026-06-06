use tracing::debug;

use super::{TeammateManager, is_settled};
use crate::error::TeamError;
use crate::types::{MailboxMessageType, TaskStatus, TeammateRole, TeammateStatus};

impl TeammateManager {
    pub async fn set_status(&self, slot_id: &str, status: TeammateStatus) -> Result<(), TeamError> {
        {
            let mut slots = self.slots.lock().await;
            let slot = slots
                .get_mut(slot_id)
                .ok_or_else(|| TeamError::AgentNotFound(slot_id.to_owned()))?;
            slot.status = status;
            slot.agent.status = Some(status);
        }
        self.events.broadcast_agent_status(slot_id, status);
        debug!(team_id = %self.team_id, slot_id, %status, "agent status changed");
        Ok(())
    }

    pub async fn get_status(&self, slot_id: &str) -> Result<TeammateStatus, TeamError> {
        let slots = self.slots.lock().await;
        let slot = slots
            .get(slot_id)
            .ok_or_else(|| TeamError::AgentNotFound(slot_id.to_owned()))?;
        Ok(slot.status)
    }

    pub async fn try_wake(&self, slot_id: &str) -> Result<Option<super::WakePayload>, TeamError> {
        let current = self.get_status(slot_id).await?;
        if current != TeammateStatus::Idle {
            debug!(
                team_id = %self.team_id,
                slot_id,
                current_status = %current,
                "skip wake: agent not idle"
            );
            return Ok(None);
        }
        self.set_status(slot_id, TeammateStatus::Working).await?;
        let payload = self.build_wake_payload(slot_id).await?;
        Ok(Some(payload))
    }

    pub async fn mark_idle(&self, slot_id: &str, summary: Option<&str>) -> Result<Option<String>, TeamError> {
        self.set_status(slot_id, TeammateStatus::Idle).await?;

        let is_lead = {
            let slots = self.slots.lock().await;
            let slot = slots
                .get(slot_id)
                .ok_or_else(|| TeamError::AgentNotFound(slot_id.to_owned()))?;
            slot.agent.role == TeammateRole::Lead
        };

        if is_lead {
            return Ok(None);
        }

        if let Some(lead_slot_id) = self.find_lead_slot_id().await
            && lead_slot_id != slot_id
        {
            self.mailbox
                .write(
                    &self.team_id,
                    &lead_slot_id,
                    slot_id,
                    MailboxMessageType::IdleNotification,
                    summary.unwrap_or("idle"),
                    summary,
                )
                .await?;
        }

        self.maybe_wake_leader_when_all_idle().await
    }

    pub async fn take_needs_role_prompt(&self, slot_id: &str) -> bool {
        let mut slots = self.slots.lock().await;
        if let Some(slot) = slots.get_mut(slot_id) {
            let needed = slot.needs_role_prompt;
            slot.needs_role_prompt = false;
            needed
        } else {
            false
        }
    }

    // Foundry: Phase 2 (auto-routing)
    /// After a task transitions to `Completed`, autonomously route any of its
    /// downstream tasks that are now fully unblocked (empty `blocked_by`),
    /// still `Pending`, and have an assigned `owner` to that owner.
    ///
    /// For each such owner this writes a "work your assigned task" entry into
    /// their mailbox (the mailbox is the source of truth; the caller is
    /// responsible for the `EventLoopRegistry::notify` poke afterwards, exactly
    /// like the `send_message` / `shutdown_request` wake paths). Owners that
    /// are not currently `Idle` are skipped here for the mailbox write *and*
    /// the returned wake list — the existing turn-claim guard in the event loop
    /// would already reject a wake against a `Working` agent, but skipping early
    /// avoids piling redundant mailbox rows on a busy agent.
    ///
    /// Returns the slot_ids that were woken (so the caller can poke their event
    /// loops). The `from_slot_id` is the agent that completed the upstream task
    /// (recorded as the mailbox sender); pass `"system"` when unknown.
    ///
    /// Behavior note: this is intentionally idempotent-friendly — re-running it
    /// for the same completed task would re-notify only owners still `Idle` with
    /// a pending unblocked task, which the wake/turn-claim guard tolerates.
    pub async fn route_unblocked_owners(
        &self,
        completed_task_id: &str,
        from_slot_id: &str,
    ) -> Result<Vec<String>, TeamError> {
        let tasks = self.task_board.list_tasks(&self.team_id).await?;

        // Downstream task ids the completed task was blocking.
        let Some(completed) = tasks.iter().find(|t| t.id == completed_task_id) else {
            return Ok(Vec::new());
        };
        let downstream: Vec<String> = completed.blocks.clone();
        if downstream.is_empty() {
            return Ok(Vec::new());
        }

        let mut woken: Vec<String> = Vec::new();
        for task in &tasks {
            if !downstream.contains(&task.id) {
                continue;
            }
            // Must be fully unblocked, still pending, and owned.
            if !task.blocked_by.is_empty() || task.status != TaskStatus::Pending {
                continue;
            }
            let Some(owner) = task.owner.as_deref() else {
                continue;
            };

            // Guard: only wake an Idle owner. A Working owner is mid-turn and
            // will pick the task up from its board on the next drain.
            let owner_status = match self.get_status(owner).await {
                Ok(s) => s,
                // Owner may no longer be a live slot (removed/renamed). Skip.
                Err(_) => continue,
            };
            if owner_status != TeammateStatus::Idle {
                debug!(
                    team_id = %self.team_id,
                    owner,
                    status = %owner_status,
                    task_id = %task.id,
                    "auto-routing: owner not idle, skipping wake"
                );
                continue;
            }

            let short = if task.id.len() > 8 { &task.id[..8] } else { &task.id };
            let content = format!(
                "A prerequisite just completed — your assigned task is now unblocked. \
                 Work your assigned task \"{}\" (id {}…). Use team_task_update to mark it in_progress.",
                task.subject, short,
            );
            self.mailbox
                .write(
                    &self.team_id,
                    owner,
                    from_slot_id,
                    MailboxMessageType::Message,
                    &content,
                    None,
                )
                .await?;
            if !woken.contains(&owner.to_owned()) {
                woken.push(owner.to_owned());
            }
            debug!(
                team_id = %self.team_id,
                owner,
                task_id = %task.id,
                "auto-routing: woke owner for unblocked task"
            );
        }

        Ok(woken)
    }

    pub(crate) async fn maybe_wake_leader_when_all_idle(&self) -> Result<Option<String>, TeamError> {
        let slots = self.slots.lock().await;

        let mut lead_slot_id = None;
        let mut all_teammates_settled = true;
        let mut has_teammates = false;

        for slot in slots.values() {
            if slot.agent.role == TeammateRole::Lead {
                lead_slot_id = Some(slot.agent.slot_id.clone());
                continue;
            }
            has_teammates = true;
            if !is_settled(slot.status) {
                all_teammates_settled = false;
                break;
            }
        }

        let Some(lead_id) = lead_slot_id else {
            return Ok(None);
        };

        if !has_teammates {
            return Ok(None);
        }

        if !all_teammates_settled {
            return Ok(None);
        }

        let lead_is_idle = slots
            .get(&lead_id)
            .map(|s| s.status == TeammateStatus::Idle)
            .unwrap_or(false);

        if !lead_is_idle {
            return Ok(None);
        }

        drop(slots);

        debug!(
            team_id = %self.team_id,
            lead_slot_id = %lead_id,
            "all teammates settled — signaling to wake leader"
        );

        Ok(Some(lead_id))
    }
}

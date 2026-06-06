// Foundry: Phase 3 (multi-project)
use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

/// Foundry: Phase 3 (multi-project)
/// Row mapping for the `projects` table.
///
/// A project is a user-owned grouping that conversations and teams can be
/// associated with via their optional `project_id` columns.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

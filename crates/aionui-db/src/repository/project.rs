// Foundry: Phase 3 (multi-project)
use crate::error::DbError;
use crate::models::ProjectRow;

/// Foundry: Phase 3 (multi-project)
/// Parameters for updating a project record.
///
/// All fields are optional; `None` means "keep the current value".
/// `description` uses double-`Option` so callers can explicitly clear it
/// (`Some(None)`) versus leave it untouched (`None`).
#[derive(Debug, Clone, Default)]
pub struct UpdateProjectParams {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

/// Foundry: Phase 3 (multi-project)
/// Data access abstraction for the `projects` table.
///
/// Object-safe via `async_trait` to support `Arc<dyn IProjectRepository>`.
#[async_trait::async_trait]
pub trait IProjectRepository: Send + Sync {
    /// Inserts a new project row.
    async fn create(&self, row: &ProjectRow) -> Result<(), DbError>;

    /// Returns a single project by id, or `None` if not found.
    async fn get(&self, id: &str) -> Result<Option<ProjectRow>, DbError>;

    /// Returns all projects owned by a user, ordered by creation time ascending.
    async fn list_by_user(&self, user_id: &str) -> Result<Vec<ProjectRow>, DbError>;

    /// Updates a project by id with the provided fields.
    /// Returns `DbError::NotFound` if absent.
    async fn update(&self, id: &str, params: &UpdateProjectParams) -> Result<(), DbError>;

    /// Deletes a project by id. Returns `DbError::NotFound` if absent.
    /// Conversations/teams referencing it are detached via `ON DELETE SET NULL`.
    async fn delete(&self, id: &str) -> Result<(), DbError>;
}

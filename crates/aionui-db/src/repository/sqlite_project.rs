// Foundry: Phase 3 (multi-project)
use aionui_common::now_ms;
use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::ProjectRow;
use crate::repository::project::{IProjectRepository, UpdateProjectParams};

/// Foundry: Phase 3 (multi-project)
/// SQLite-backed implementation of [`IProjectRepository`].
#[derive(Clone, Debug)]
pub struct SqliteProjectRepository {
    pool: SqlitePool,
}

impl SqliteProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl IProjectRepository for SqliteProjectRepository {
    async fn create(&self, row: &ProjectRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO projects (id, user_id, name, description, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&row.id)
        .bind(&row.user_id)
        .bind(&row.name)
        .bind(&row.description)
        .bind(row.created_at)
        .bind(row.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<ProjectRow>, DbError> {
        let row = sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    async fn list_by_user(&self, user_id: &str) -> Result<Vec<ProjectRow>, DbError> {
        let rows = sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE user_id = ? ORDER BY created_at ASC")
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    async fn update(&self, id: &str, params: &UpdateProjectParams) -> Result<(), DbError> {
        let mut set_clauses = Vec::new();
        if params.name.is_some() {
            set_clauses.push("name = ?");
        }
        if params.description.is_some() {
            set_clauses.push("description = ?");
        }

        if set_clauses.is_empty() {
            return Ok(());
        }

        set_clauses.push("updated_at = ?");
        let sql = format!("UPDATE projects SET {} WHERE id = ?", set_clauses.join(", "));

        let mut query = sqlx::query(&sql);
        if let Some(ref name) = params.name {
            query = query.bind(name);
        }
        if let Some(ref description) = params.description {
            query = query.bind(description.as_deref());
        }
        query = query.bind(now_ms());
        query = query.bind(id);

        let result = query.execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("project {id}")));
        }
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("project {id}")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_database_memory;

    const SYSTEM_USER_ID: &str = "system_default_user";

    async fn setup() -> (SqliteProjectRepository, crate::Database) {
        let db = init_database_memory().await.unwrap();
        let repo = SqliteProjectRepository::new(db.pool().clone());
        (repo, db)
    }

    fn sample_project(user_id: &str) -> ProjectRow {
        let now = aionui_common::now_ms();
        ProjectRow {
            id: aionui_common::generate_prefixed_id("proj"),
            user_id: user_id.to_string(),
            name: "Test Project".to_string(),
            description: Some("A project".to_string()),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn create_and_get_project() {
        let (repo, _db) = setup().await;
        let project = sample_project(SYSTEM_USER_ID);

        repo.create(&project).await.unwrap();
        let found = repo.get(&project.id).await.unwrap().unwrap();

        assert_eq!(found.id, project.id);
        assert_eq!(found.name, "Test Project");
        assert_eq!(found.description.as_deref(), Some("A project"));
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let (repo, _db) = setup().await;
        assert!(repo.get("no_such_id").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_by_user_ordered_by_created_at() {
        let (repo, _db) = setup().await;

        let mut p1 = sample_project(SYSTEM_USER_ID);
        p1.name = "First".to_string();
        p1.created_at = 1000;
        repo.create(&p1).await.unwrap();

        let mut p2 = sample_project(SYSTEM_USER_ID);
        p2.name = "Second".to_string();
        p2.created_at = 2000;
        repo.create(&p2).await.unwrap();

        let rows = repo.list_by_user(SYSTEM_USER_ID).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "First");
        assert_eq!(rows[1].name, "Second");
    }

    #[tokio::test]
    async fn update_project_name_and_description() {
        let (repo, _db) = setup().await;
        let project = sample_project(SYSTEM_USER_ID);
        repo.create(&project).await.unwrap();

        repo.update(
            &project.id,
            &UpdateProjectParams {
                name: Some("Renamed".to_string()),
                description: Some(None),
            },
        )
        .await
        .unwrap();

        let found = repo.get(&project.id).await.unwrap().unwrap();
        assert_eq!(found.name, "Renamed");
        assert!(found.description.is_none());
    }

    #[tokio::test]
    async fn update_empty_is_noop() {
        let (repo, _db) = setup().await;
        let project = sample_project(SYSTEM_USER_ID);
        repo.create(&project).await.unwrap();
        repo.update(&project.id, &UpdateProjectParams::default()).await.unwrap();
    }

    #[tokio::test]
    async fn update_nonexistent_returns_not_found() {
        let (repo, _db) = setup().await;
        let err = repo
            .update(
                "no_id",
                &UpdateProjectParams {
                    name: Some("x".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[tokio::test]
    async fn delete_project() {
        let (repo, _db) = setup().await;
        let project = sample_project(SYSTEM_USER_ID);
        repo.create(&project).await.unwrap();

        repo.delete(&project.id).await.unwrap();
        assert!(repo.get(&project.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let (repo, _db) = setup().await;
        let err = repo.delete("no_id").await.unwrap_err();
        assert!(matches!(err, DbError::NotFound(_)));
    }
}

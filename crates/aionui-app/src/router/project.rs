// Foundry: Phase 3 (multi-project)
//! Project CRUD routes (`/api/projects`).
//!
//! Self-contained module mirroring how `team_routes` is built and mounted:
//! a small `ProjectRouterState` carrying the repository, a `project_routes`
//! builder, and handlers using `State` + `Extension<CurrentUser>` +
//! `Path`/`Json`, returning `Json<ApiResponse<...>>`.

use std::sync::Arc;

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch};

use aionui_api_types::{
    ApiResponse, CreateProjectRequest, ProjectListResponse, ProjectResponse, UpdateProjectRequest,
};
use aionui_auth::CurrentUser;
use aionui_common::{ApiError, generate_prefixed_id, now_ms};
use aionui_db::{DbError, IProjectRepository, ProjectRow, UpdateProjectParams};

/// Foundry: Phase 3 (multi-project)
/// Router state for the project endpoints.
#[derive(Clone)]
pub struct ProjectRouterState {
    pub repo: Arc<dyn IProjectRepository>,
}

/// Foundry: Phase 3 (multi-project)
/// Maps a `DbError` to the API boundary error, mirroring the team route mapper.
fn db_error_to_api_error(err: DbError) -> ApiError {
    match err {
        DbError::NotFound(msg) => ApiError::NotFound(msg),
        DbError::Conflict(msg) => ApiError::Conflict(msg),
        DbError::Query(e) => ApiError::Internal(format!("Database error: {e}")),
        DbError::Migration(e) => ApiError::Internal(format!("Migration error: {e}")),
        DbError::Init(msg) => ApiError::Internal(format!("Database init error: {msg}")),
    }
}

/// Foundry: Phase 3 (multi-project)
fn project_row_to_response(row: ProjectRow) -> ProjectResponse {
    ProjectResponse {
        id: row.id,
        name: row.name,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// Foundry: Phase 3 (multi-project)
/// Build the project router. Mounted in `routes.rs` behind auth middleware.
pub fn project_routes(state: ProjectRouterState) -> Router {
    Router::new()
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/projects/{id}", patch(update_project).delete(delete_project))
        .with_state(state)
}

/// Foundry: Phase 3 (multi-project)
async fn list_projects(
    State(state): State<ProjectRouterState>,
    Extension(user): Extension<CurrentUser>,
) -> Result<Json<ApiResponse<ProjectListResponse>>, ApiError> {
    let rows = state.repo.list_by_user(&user.id).await.map_err(db_error_to_api_error)?;
    let projects = rows.into_iter().map(project_row_to_response).collect();
    Ok(Json(ApiResponse::ok(ProjectListResponse { projects })))
}

/// Foundry: Phase 3 (multi-project)
async fn create_project(
    State(state): State<ProjectRouterState>,
    Extension(user): Extension<CurrentUser>,
    body: Result<Json<CreateProjectRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<ApiResponse<ProjectResponse>>), ApiError> {
    let Json(req) = body.map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let now = now_ms();
    let row = ProjectRow {
        id: generate_prefixed_id("proj"),
        user_id: user.id.clone(),
        name: req.name,
        description: req.description,
        created_at: now,
        updated_at: now,
    };
    state.repo.create(&row).await.map_err(db_error_to_api_error)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(project_row_to_response(row)))))
}

/// Foundry: Phase 3 (multi-project)
async fn update_project(
    State(state): State<ProjectRouterState>,
    Extension(_user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<UpdateProjectRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProjectResponse>>, ApiError> {
    let Json(req) = body.map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let params = UpdateProjectParams {
        name: req.name,
        // A present `description` key sets the value; absent leaves it unchanged.
        description: req.description.map(Some),
    };
    state.repo.update(&id, &params).await.map_err(db_error_to_api_error)?;
    let row = state
        .repo
        .get(&id)
        .await
        .map_err(db_error_to_api_error)?
        .ok_or_else(|| ApiError::NotFound(format!("project {id}")))?;
    Ok(Json(ApiResponse::ok(project_row_to_response(row))))
}

/// Foundry: Phase 3 (multi-project)
async fn delete_project(
    State(state): State<ProjectRouterState>,
    Extension(_user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    state.repo.delete(&id).await.map_err(db_error_to_api_error)?;
    Ok(Json(ApiResponse::success()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_router_state_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<ProjectRouterState>();
    }

    #[test]
    fn db_not_found_maps_to_api_not_found() {
        let err = db_error_to_api_error(DbError::NotFound("project p1".into()));
        assert!(matches!(err, ApiError::NotFound(msg) if msg == "project p1"));
    }

    #[test]
    fn project_row_maps_to_response() {
        let row = ProjectRow {
            id: "proj-1".into(),
            user_id: "u1".into(),
            name: "Alpha".into(),
            description: Some("d".into()),
            created_at: 1,
            updated_at: 2,
        };
        let resp = project_row_to_response(row);
        assert_eq!(resp.id, "proj-1");
        assert_eq!(resp.name, "Alpha");
        assert_eq!(resp.description.as_deref(), Some("d"));
    }
}

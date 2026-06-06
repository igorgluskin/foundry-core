// Foundry: Phase 3 (multi-project)
use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

/// Foundry: Phase 3 (multi-project)
/// Full project object returned by create, get, list, and update endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

/// Foundry: Phase 3 (multi-project)
/// Response body for `GET /api/projects`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}

/// Foundry: Phase 3 (multi-project)
/// Request body for `POST /api/projects`.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Foundry: Phase 3 (multi-project)
/// Request body for `PATCH /api/projects/{id}`.
///
/// All fields optional — only supplied fields are applied.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_project_response() -> ProjectResponse {
        ProjectResponse {
            id: "proj-1".into(),
            name: "Alpha".into(),
            description: Some("desc".into()),
            created_at: 1000,
            updated_at: 2000,
        }
    }

    #[test]
    fn serialize_project_response_snake_case() {
        let json = serde_json::to_value(sample_project_response()).unwrap();
        assert_eq!(json["id"], "proj-1");
        assert_eq!(json["name"], "Alpha");
        assert_eq!(json["description"], "desc");
        assert_eq!(json["created_at"], 1000_i64);
        assert_eq!(json["updated_at"], 2000_i64);
    }

    #[test]
    fn serialize_project_response_optional_description_omitted() {
        let project = ProjectResponse {
            description: None,
            ..sample_project_response()
        };
        let json = serde_json::to_value(&project).unwrap();
        assert!(json.get("description").is_none());
    }

    #[test]
    fn project_response_roundtrip() {
        let project = sample_project_response();
        let parsed: ProjectResponse = serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        assert_eq!(parsed, project);
    }

    #[test]
    fn serialize_project_list_response() {
        let list = ProjectListResponse {
            projects: vec![sample_project_response()],
        };
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(json["projects"].as_array().unwrap().len(), 1);
        assert_eq!(json["projects"][0]["id"], "proj-1");
    }

    #[test]
    fn deserialize_create_project_request_full() {
        let raw = json!({ "name": "New", "description": "Details" });
        let req: CreateProjectRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name, "New");
        assert_eq!(req.description.as_deref(), Some("Details"));
    }

    #[test]
    fn deserialize_create_project_request_minimal() {
        let raw = json!({ "name": "Just name" });
        let req: CreateProjectRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name, "Just name");
        assert!(req.description.is_none());
    }

    #[test]
    fn deserialize_create_project_request_missing_name() {
        let raw = json!({ "description": "no name" });
        let result = serde_json::from_value::<CreateProjectRequest>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_update_project_request_full() {
        let raw = json!({ "name": "Renamed", "description": "New desc" });
        let req: UpdateProjectRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.name.as_deref(), Some("Renamed"));
        assert_eq!(req.description.as_deref(), Some("New desc"));
    }

    #[test]
    fn deserialize_update_project_request_empty() {
        let raw = json!({});
        let req: UpdateProjectRequest = serde_json::from_value(raw).unwrap();
        assert!(req.name.is_none());
        assert!(req.description.is_none());
    }
}

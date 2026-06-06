-- Foundry: Phase 3 (multi-project)
-- Migration 010: Multi-project layer.
--
-- Adds a `projects` table owned by a user and links conversations and teams
-- to an optional project. All changes are additive: existing rows keep
-- `project_id = NULL` and continue to behave exactly as before.
--
-- `users(id)` is the primary key of the `users` table (see 001_initial_schema.sql).
-- Deleting a user cascades to their projects; deleting a project detaches
-- (SET NULL) any conversations/teams that referenced it rather than removing them.

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_projects_user_id ON projects(user_id);

ALTER TABLE conversations ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;
ALTER TABLE teams ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_conversations_project_id ON conversations(project_id);
CREATE INDEX IF NOT EXISTS idx_teams_project_id ON teams(project_id);

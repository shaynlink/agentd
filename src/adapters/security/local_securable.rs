use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use rusqlite::types::Value as SqlValue;
use serde_json::Value;

use crate::config::SandboxProviderConfig;
use crate::domain::permission::RuntimeRole;
use crate::ports::securable::{
    AuditEventFilters, RbacBindingRecord, RbacPolicyRecord, RbacPolicySpec, RbacRolePolicyRecord,
    RbacRoleRecord, RbacSnapshot, SecurablePort,
};

pub struct LocalSecurable {
    allowed_commands: Vec<String>,
    allowed_read_paths: Vec<String>,
    allowed_write_paths: Vec<String>,
    audit_log_path: PathBuf,
    audit_backend: String,
}

impl LocalSecurable {
    pub fn new(config: &SandboxProviderConfig) -> Self {
        Self {
            allowed_commands: config.allowed_commands.clone(),
            allowed_read_paths: config.allowed_read_paths.clone(),
            allowed_write_paths: config.allowed_write_paths.clone(),
            audit_log_path: config.audit_log_path.clone(),
            audit_backend: config.audit_backend.clone(),
        }
    }

    fn open_sqlite(&self) -> Result<Connection> {
        if let Some(parent) = self.audit_log_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create sqlite audit dir: {}", parent.display())
            })?;
        }

        Connection::open(&self.audit_log_path).with_context(|| {
            format!(
                "failed to open sqlite audit DB: {}",
                self.audit_log_path.display()
            )
        })
    }

    fn ensure_sqlite_rbac_schema(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS rbac_roles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                is_builtin INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS rbac_policies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                resource_type TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_pattern TEXT NOT NULL,
                effect TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS rbac_role_policies (
                role_id INTEGER NOT NULL,
                policy_id INTEGER NOT NULL,
                PRIMARY KEY(role_id, policy_id),
                FOREIGN KEY(role_id) REFERENCES rbac_roles(id),
                FOREIGN KEY(policy_id) REFERENCES rbac_policies(id)
            );

            CREATE TABLE IF NOT EXISTS rbac_bindings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                subject_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                role_id INTEGER NOT NULL,
                UNIQUE(subject_type, subject, role_id),
                FOREIGN KEY(role_id) REFERENCES rbac_roles(id)
            );

            CREATE INDEX IF NOT EXISTS idx_rbac_bindings_subject
                ON rbac_bindings(subject_type, subject);
            CREATE INDEX IF NOT EXISTS idx_rbac_policies_resource_action
                ON rbac_policies(resource_type, action);
            "#,
        )
        .context("failed to initialize sqlite RBAC schema")?;

        Ok(())
    }

    fn ensure_sqlite_audit_and_rbac_schema(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS security_audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                payload TEXT NOT NULL,
                agent_id TEXT,
                role TEXT,
                runtime TEXT,
                allowed INTEGER,
                exit_code INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_security_audit_logs_ts
                ON security_audit_logs(ts);
            CREATE INDEX IF NOT EXISTS idx_security_audit_logs_agent_id
                ON security_audit_logs(agent_id);
            CREATE INDEX IF NOT EXISTS idx_security_audit_logs_role
                ON security_audit_logs(role);
            CREATE INDEX IF NOT EXISTS idx_security_audit_logs_runtime
                ON security_audit_logs(runtime);
            CREATE INDEX IF NOT EXISTS idx_security_audit_logs_allowed
                ON security_audit_logs(allowed);
            "#,
        )
        .context("failed to initialize sqlite audit schema")?;

        self.ensure_sqlite_rbac_schema(conn)?;

        Ok(())
    }

    fn get_or_create_role_id(
        &self,
        conn: &Connection,
        role_name: &str,
        description: &str,
    ) -> Result<i64> {
        conn.execute(
            "INSERT OR IGNORE INTO rbac_roles (name, description, is_builtin) VALUES (?1, ?2, 1)",
            params![role_name, description],
        )?;
        let id = conn.query_row(
            "SELECT id FROM rbac_roles WHERE LOWER(name) = LOWER(?1)",
            params![role_name],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    fn get_or_create_policy_id(
        &self,
        conn: &Connection,
        name: &str,
        resource_type: &str,
        action: &str,
        resource_pattern: &str,
        effect: &str,
    ) -> Result<i64> {
        conn.execute(
            "INSERT OR IGNORE INTO rbac_policies (name, resource_type, action, resource_pattern, effect)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, resource_type, action, resource_pattern, effect],
        )?;
        let id = conn.query_row(
            "SELECT id FROM rbac_policies WHERE name = ?1",
            params![name],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    fn bind_role_to_subject(
        &self,
        conn: &Connection,
        subject_type: &str,
        subject: &str,
        role_id: i64,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO rbac_bindings (subject_type, subject, role_id) VALUES (?1, ?2, ?3)",
            params![subject_type, subject, role_id],
        )?;
        Ok(())
    }

    fn attach_policy_to_role(&self, conn: &Connection, role_id: i64, policy_id: i64) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO rbac_role_policies (role_id, policy_id) VALUES (?1, ?2)",
            params![role_id, policy_id],
        )?;
        Ok(())
    }

    fn ensure_default_rbac_seed(&self, conn: &Connection) -> Result<()> {
        let role_count: i64 = conn.query_row("SELECT COUNT(*) FROM rbac_roles", [], |row| row.get(0))?;
        if role_count > 0 {
            return Ok(());
        }

        let admin_role_id = self.get_or_create_role_id(conn, "admin", "Built-in admin role")?;
        let operator_role_id =
            self.get_or_create_role_id(conn, "operator", "Built-in operator role")?;
        let viewer_role_id = self.get_or_create_role_id(conn, "viewer", "Built-in viewer role")?;

        self.bind_role_to_subject(conn, "runtime_role", "admin", admin_role_id)?;
        self.bind_role_to_subject(conn, "runtime_role", "operator", operator_role_id)?;
        self.bind_role_to_subject(conn, "runtime_role", "viewer", viewer_role_id)?;

        let admin_cmd_allow = self.get_or_create_policy_id(
            conn,
            "admin.command.execute.allow_all",
            "command",
            "execute",
            "*",
            "allow",
        )?;
        let admin_path_allow = self.get_or_create_policy_id(
            conn,
            "admin.path.access.allow_all",
            "path",
            "access",
            "*",
            "allow",
        )?;
        self.attach_policy_to_role(conn, admin_role_id, admin_cmd_allow)?;
        self.attach_policy_to_role(conn, admin_role_id, admin_path_allow)?;

        if self.allowed_commands.is_empty() {
            let operator_cmd_allow = self.get_or_create_policy_id(
                conn,
                "operator.command.execute.allow_all",
                "command",
                "execute",
                "*",
                "allow",
            )?;
            self.attach_policy_to_role(conn, operator_role_id, operator_cmd_allow)?;
        } else {
            for (idx, command) in self.allowed_commands.iter().enumerate() {
                let policy_name = format!("operator.command.execute.allow.{}", idx + 1);
                let operator_cmd_allow = self.get_or_create_policy_id(
                    conn,
                    &policy_name,
                    "command",
                    "execute",
                    command,
                    "allow",
                )?;
                self.attach_policy_to_role(conn, operator_role_id, operator_cmd_allow)?;
            }
        }

        let mut allowed_paths = Vec::new();
        allowed_paths.extend(self.allowed_read_paths.clone());
        allowed_paths.extend(self.allowed_write_paths.clone());

        if allowed_paths.is_empty() {
            let op_path_allow = self.get_or_create_policy_id(
                conn,
                "operator.path.access.allow_all",
                "path",
                "access",
                "*",
                "allow",
            )?;
            let viewer_path_allow = self.get_or_create_policy_id(
                conn,
                "viewer.path.access.allow_all",
                "path",
                "access",
                "*",
                "allow",
            )?;
            self.attach_policy_to_role(conn, operator_role_id, op_path_allow)?;
            self.attach_policy_to_role(conn, viewer_role_id, viewer_path_allow)?;
        } else {
            for (idx, path) in allowed_paths.iter().enumerate() {
                let op_policy_name = format!("operator.path.access.allow.{}", idx + 1);
                let viewer_policy_name = format!("viewer.path.access.allow.{}", idx + 1);

                let op_path_allow = self.get_or_create_policy_id(
                    conn,
                    &op_policy_name,
                    "path",
                    "access",
                    path,
                    "allow",
                )?;
                let viewer_path_allow = self.get_or_create_policy_id(
                    conn,
                    &viewer_policy_name,
                    "path",
                    "access",
                    path,
                    "allow",
                )?;

                self.attach_policy_to_role(conn, operator_role_id, op_path_allow)?;
                self.attach_policy_to_role(conn, viewer_role_id, viewer_path_allow)?;
            }
        }

        let viewer_cmd_deny = self.get_or_create_policy_id(
            conn,
            "viewer.command.execute.deny_all",
            "command",
            "execute",
            "*",
            "deny",
        )?;
        self.attach_policy_to_role(conn, viewer_role_id, viewer_cmd_deny)?;

        Ok(())
    }

    fn ensure_sqlite_rbac_ready(&self) -> Result<Connection> {
        if !self.audit_backend.eq_ignore_ascii_case("sqlite") {
            anyhow::bail!(
                "RBAC management requires sqlite audit backend (set AGENTD_SANDBOX_AUDIT_BACKEND=sqlite)"
            );
        }

        let conn = self.open_sqlite()?;
        self.ensure_sqlite_rbac_schema(&conn)?;
        self.ensure_default_rbac_seed(&conn)?;
        Ok(conn)
    }

    fn role_id_by_name(&self, conn: &Connection, role_name: &str) -> Result<Option<i64>> {
        let mut stmt = conn.prepare("SELECT id FROM rbac_roles WHERE LOWER(name) = LOWER(?1)")?;
        let mut rows = stmt.query(params![role_name])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get::<_, i64>(0)?));
        }
        Ok(None)
    }

    fn matches_pattern(pattern: &str, candidate: &str) -> bool {
        let pattern = pattern.to_ascii_lowercase();
        let candidate = candidate.to_ascii_lowercase();

        if pattern == "*" {
            return true;
        }

        if !pattern.contains('*') {
            return candidate == pattern;
        }

        let starts_with_wildcard = pattern.starts_with('*');
        let ends_with_wildcard = pattern.ends_with('*');
        let chunks: Vec<&str> = pattern.split('*').filter(|chunk| !chunk.is_empty()).collect();

        if chunks.is_empty() {
            return true;
        }

        let mut offset = 0_usize;
        for (idx, chunk) in chunks.iter().enumerate() {
            if idx == 0 && !starts_with_wildcard {
                if !candidate.starts_with(chunk) {
                    return false;
                }
                offset = chunk.len();
                continue;
            }

            if let Some(found) = candidate[offset..].find(chunk) {
                offset += found + chunk.len();
            } else {
                return false;
            }
        }

        if !ends_with_wildcard
            && let Some(last) = chunks.last()
        {
            return candidate.ends_with(last);
        }

        true
    }

    fn evaluate_persisted_policies(
        &self,
        conn: &Connection,
        role_subject: &str,
        resource_type: &str,
        action: &str,
        candidate: &str,
    ) -> Result<Option<bool>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT p.resource_pattern, p.effect
            FROM rbac_bindings b
            JOIN rbac_role_policies rp ON rp.role_id = b.role_id
            JOIN rbac_policies p ON p.id = rp.policy_id
            WHERE LOWER(b.subject_type) = 'runtime_role'
              AND LOWER(b.subject) = LOWER(?1)
              AND LOWER(p.resource_type) = LOWER(?2)
              AND LOWER(p.action) = LOWER(?3)
            "#,
        )?;

        let rows = stmt.query_map(params![role_subject, resource_type, action], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut scoped_policy_count = 0_usize;
        let mut matched_allow = false;
        let mut matched_any = false;

        for row in rows {
            let (pattern, effect) = row?;
            scoped_policy_count += 1;
            if !Self::matches_pattern(&pattern, candidate) {
                continue;
            }

            matched_any = true;
            if effect.eq_ignore_ascii_case("deny") {
                return Ok(Some(false));
            }
            if effect.eq_ignore_ascii_case("allow") {
                matched_allow = true;
            }
        }

        if matched_any {
            return Ok(Some(matched_allow));
        }

        // If the subject is bound to RBAC policies for this resource/action,
        // enforce default deny when no policy pattern matches.
        if scoped_policy_count > 0 {
            return Ok(Some(false));
        }

        Ok(None)
    }

    fn write_file_audit(&self, payload: &str) -> Result<()> {
        if let Some(parent) = self.audit_log_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create audit log dir: {}", parent.display()))?;
        }

        let mut existing = if self.audit_log_path.exists() {
            fs::read_to_string(&self.audit_log_path).with_context(|| {
                format!(
                    "failed to read audit log file: {}",
                    self.audit_log_path.display()
                )
            })?
        } else {
            String::new()
        };

        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(payload);
        existing.push('\n');

        fs::write(&self.audit_log_path, existing).with_context(|| {
            format!(
                "failed to write audit log file: {}",
                self.audit_log_path.display()
            )
        })?;

        Ok(())
    }

    fn write_sqlite_audit(&self, payload: &str) -> Result<()> {
        let conn = self.open_sqlite()?;
        self.ensure_sqlite_audit_and_rbac_schema(&conn)?;
        self.ensure_default_rbac_seed(&conn)?;

        // Migration-safe upgrade for pre-existing DBs created before normalized columns.
        let existing_cols = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(security_audit_logs)")
                .context("failed to inspect security_audit_logs schema")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .context("failed to read security_audit_logs columns")?;
            let mut cols = std::collections::HashSet::new();
            for row in rows {
                cols.insert(row?);
            }
            cols
        };

        for column_sql in [
            (
                "agent_id",
                "ALTER TABLE security_audit_logs ADD COLUMN agent_id TEXT",
            ),
            (
                "role",
                "ALTER TABLE security_audit_logs ADD COLUMN role TEXT",
            ),
            (
                "runtime",
                "ALTER TABLE security_audit_logs ADD COLUMN runtime TEXT",
            ),
            (
                "allowed",
                "ALTER TABLE security_audit_logs ADD COLUMN allowed INTEGER",
            ),
            (
                "exit_code",
                "ALTER TABLE security_audit_logs ADD COLUMN exit_code INTEGER",
            ),
        ] {
            if !existing_cols.contains(column_sql.0) {
                conn.execute(column_sql.1, [])
                    .with_context(|| format!("failed to migrate audit column: {}", column_sql.0))?;
            }
        }

        let parsed = serde_json::from_str::<Value>(payload).ok();
        let ts = parsed
            .as_ref()
            .and_then(|v| v.get("ts"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let agent_id = parsed
            .as_ref()
            .and_then(|v| v.get("agent_id"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let role = parsed
            .as_ref()
            .and_then(|v| v.get("role"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let runtime = parsed
            .as_ref()
            .and_then(|v| v.get("runtime"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let allowed = parsed
            .as_ref()
            .and_then(|v| v.get("allowed"))
            .and_then(Value::as_bool)
            .map(|b| if b { 1_i64 } else { 0_i64 });
        let exit_code = parsed
            .as_ref()
            .and_then(|v| v.get("exit_code"))
            .and_then(Value::as_i64);

        conn.execute(
            "INSERT INTO security_audit_logs (ts, payload, agent_id, role, runtime, allowed, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![ts, payload, agent_id, role, runtime, allowed, exit_code],
        )
        .context("failed to append sqlite audit log")?;

        Ok(())
    }

    fn payload_matches_filters(payload: &str, filters: AuditEventFilters<'_>) -> bool {
        let parsed = match serde_json::from_str::<Value>(payload) {
            Ok(v) => v,
            Err(_) => return false,
        };

        if let Some(role_filter) = filters.role {
            let matches = parsed
                .get("role")
                .and_then(Value::as_str)
                .map(|r| r.eq_ignore_ascii_case(role_filter))
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }

        if let Some(allowed_filter) = filters.allowed {
            let matches = parsed
                .get("allowed")
                .and_then(Value::as_bool)
                .map(|v| v == allowed_filter)
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }

        if let Some(runtime_filter) = filters.runtime {
            let matches = parsed
                .get("runtime")
                .and_then(Value::as_str)
                .map(|r| r.eq_ignore_ascii_case(runtime_filter))
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }

        if let Some(agent_filter) = filters.agent_id {
            let matches = parsed
                .get("agent_id")
                .and_then(Value::as_str)
                .map(|id| id == agent_filter)
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }

        if let Some(since_filter) = filters.since {
            let since = match DateTime::parse_from_rfc3339(since_filter) {
                Ok(ts) => ts.with_timezone(&Utc),
                Err(_) => return false,
            };
            let matches = parsed
                .get("ts")
                .and_then(Value::as_str)
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc) >= since)
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }

        if let Some(until_filter) = filters.until {
            let until = match DateTime::parse_from_rfc3339(until_filter) {
                Ok(ts) => ts.with_timezone(&Utc),
                Err(_) => return false,
            };
            let matches = parsed
                .get("ts")
                .and_then(Value::as_str)
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc) <= until)
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }

        true
    }

    fn list_file_audit(&self, limit: usize, filters: AuditEventFilters<'_>) -> Result<Vec<String>> {
        if !self.audit_log_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.audit_log_path).with_context(|| {
            format!(
                "failed to read audit log file: {}",
                self.audit_log_path.display()
            )
        })?;

        let mut lines: Vec<String> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter(|line| Self::payload_matches_filters(line, filters))
            .map(|line| line.to_string())
            .collect();
        lines.reverse();
        lines.truncate(limit);
        Ok(lines)
    }

    fn list_sqlite_audit(
        &self,
        limit: usize,
        filters: AuditEventFilters<'_>,
    ) -> Result<Vec<String>> {
        if !self.audit_log_path.exists() {
            return Ok(Vec::new());
        }

        let conn = rusqlite::Connection::open(&self.audit_log_path).with_context(|| {
            format!(
                "failed to open sqlite audit DB: {}",
                self.audit_log_path.display()
            )
        })?;

        let mut conditions: Vec<String> = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        if let Some(role) = filters.role {
            conditions.push("LOWER(role) = LOWER(?)".to_string());
            bind_values.push(SqlValue::Text(role.to_string()));
        }
        if let Some(allowed) = filters.allowed {
            conditions.push("allowed = ?".to_string());
            bind_values.push(SqlValue::Integer(if allowed { 1 } else { 0 }));
        }
        if let Some(runtime) = filters.runtime {
            conditions.push("LOWER(runtime) = LOWER(?)".to_string());
            bind_values.push(SqlValue::Text(runtime.to_string()));
        }
        if let Some(agent_id) = filters.agent_id {
            conditions.push("agent_id = ?".to_string());
            bind_values.push(SqlValue::Text(agent_id.to_string()));
        }
        if let Some(since) = filters.since {
            conditions.push("ts >= ?".to_string());
            bind_values.push(SqlValue::Text(since.to_string()));
        }
        if let Some(until) = filters.until {
            conditions.push("ts <= ?".to_string());
            bind_values.push(SqlValue::Text(until.to_string()));
        }

        let mut query = "SELECT payload FROM security_audit_logs".to_string();
        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }
        query.push_str(" ORDER BY ts DESC, id DESC LIMIT ?");

        bind_values.push(SqlValue::Integer(limit as i64));

        let mut stmt = conn
            .prepare(&query)
            .context("failed to prepare audit list query")?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(bind_values), |row| {
                row.get::<_, String>(0)
            })
            .context("failed to query sqlite audit logs")?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn is_path_allowed(path: &Path, allowed_paths: &[String]) -> Result<bool> {
    if allowed_paths.is_empty() {
        return Ok(true);
    }

    let canonical = fs::canonicalize(path)
        .or_else(|_| {
            if let Some(parent) = path.parent() {
                fs::canonicalize(parent).map(|p| p.join(path.file_name().unwrap_or_default()))
            } else {
                fs::canonicalize(path)
            }
        })
        .context("failed to canonicalize path")?;

    for allowed in allowed_paths {
        let allowed_path = PathBuf::from(allowed);
        let allowed_canonical = fs::canonicalize(&allowed_path).or_else(|_| {
            if let Some(parent) = allowed_path.parent() {
                fs::canonicalize(parent)
                    .map(|p| p.join(allowed_path.file_name().unwrap_or_default()))
            } else {
                fs::canonicalize(&allowed_path)
            }
        })?;

        if canonical == allowed_canonical || canonical.starts_with(&allowed_canonical) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[async_trait]
impl SecurablePort for LocalSecurable {
    async fn check_command_access(&self, command: &str, role: &str) -> Result<bool> {
        if self.audit_backend.eq_ignore_ascii_case("sqlite") {
            let conn = self.open_sqlite()?;
            self.ensure_sqlite_rbac_schema(&conn)?;
            self.ensure_default_rbac_seed(&conn)?;

            if let Some(allowed) =
                self.evaluate_persisted_policies(&conn, role, "command", "execute", command)?
            {
                return Ok(allowed);
            }

            if !matches!(RuntimeRole::from_value(role), RuntimeRole::Admin)
                && !role.eq_ignore_ascii_case("operator")
                && !role.eq_ignore_ascii_case("viewer")
            {
                return Ok(false);
            }
        }

        let role = RuntimeRole::from_value(role);
        Ok(role != RuntimeRole::Viewer)
    }

    async fn check_file_access(&self, path: &Path, role: &str) -> Result<bool> {
        if self.audit_backend.eq_ignore_ascii_case("sqlite") {
            let conn = self.open_sqlite()?;
            self.ensure_sqlite_rbac_schema(&conn)?;
            self.ensure_default_rbac_seed(&conn)?;

            if let Some(allowed) = self.evaluate_persisted_policies(
                &conn,
                role,
                "path",
                "access",
                path.to_string_lossy().as_ref(),
            )? {
                return Ok(allowed);
            }

            if !matches!(RuntimeRole::from_value(role), RuntimeRole::Admin)
                && !role.eq_ignore_ascii_case("operator")
                && !role.eq_ignore_ascii_case("viewer")
            {
                return Ok(false);
            }
        }

        let role = RuntimeRole::from_value(role);
        if role == RuntimeRole::Admin {
            return Ok(true);
        }

        let mut allowed_paths = Vec::new();
        allowed_paths.extend(self.allowed_read_paths.clone());
        allowed_paths.extend(self.allowed_write_paths.clone());

        is_path_allowed(path, &allowed_paths)
    }

    async fn log_audit_event(&self, payload: &str) -> Result<()> {
        if self.audit_backend.eq_ignore_ascii_case("sqlite") {
            return self.write_sqlite_audit(payload);
        }

        self.write_file_audit(payload)
    }

    async fn list_audit_events(
        &self,
        limit: usize,
        filters: AuditEventFilters<'_>,
    ) -> Result<Vec<String>> {
        if self.audit_backend.eq_ignore_ascii_case("sqlite") {
            return self.list_sqlite_audit(limit, filters);
        }

        self.list_file_audit(limit, filters)
    }

    async fn create_role(&self, name: &str, description: Option<&str>) -> Result<()> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            anyhow::bail!("invalid role name: empty");
        }

        let conn = self.ensure_sqlite_rbac_ready()?;
        conn.execute(
            "INSERT OR IGNORE INTO rbac_roles (name, description, is_builtin) VALUES (?1, ?2, 0)",
            params![trimmed, description],
        )
        .context("failed to create RBAC role")?;

        Ok(())
    }

    async fn create_policy(&self, spec: &RbacPolicySpec) -> Result<()> {
        let name = spec.name.trim();
        let resource_type = spec.resource_type.trim();
        let action = spec.action.trim();
        let resource_pattern = spec.resource_pattern.trim();
        let effect = spec.effect.trim().to_ascii_lowercase();

        if name.is_empty()
            || resource_type.is_empty()
            || action.is_empty()
            || resource_pattern.is_empty()
        {
            anyhow::bail!("invalid policy: name/resource_type/action/resource_pattern are required");
        }
        if effect != "allow" && effect != "deny" {
            anyhow::bail!("invalid policy effect: expected allow|deny");
        }

        let conn = self.ensure_sqlite_rbac_ready()?;
        conn.execute(
            "INSERT OR IGNORE INTO rbac_policies (name, resource_type, action, resource_pattern, effect)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, resource_type, action, resource_pattern, effect],
        )
        .context("failed to create RBAC policy")?;

        Ok(())
    }

    async fn bind_role(&self, subject_type: &str, subject: &str, role_name: &str) -> Result<()> {
        let subject_type = subject_type.trim();
        let subject = subject.trim();
        let role_name = role_name.trim();

        if subject_type.is_empty() || subject.is_empty() || role_name.is_empty() {
            anyhow::bail!("invalid binding: subject_type, subject and role_name are required");
        }

        let conn = self.ensure_sqlite_rbac_ready()?;
        let Some(role_id) = self.role_id_by_name(&conn, role_name)? else {
            anyhow::bail!("RBAC role not found: {role_name}");
        };

        conn.execute(
            "INSERT OR IGNORE INTO rbac_bindings (subject_type, subject, role_id) VALUES (?1, ?2, ?3)",
            params![subject_type, subject, role_id],
        )
        .context("failed to bind RBAC role")?;

        Ok(())
    }

    async fn list_rbac(&self) -> Result<RbacSnapshot> {
        let conn = self.ensure_sqlite_rbac_ready()?;

        let roles = {
            let mut stmt = conn.prepare(
                "SELECT name, description, is_builtin FROM rbac_roles ORDER BY LOWER(name) ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(RbacRoleRecord {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    is_builtin: row.get::<_, i64>(2)? != 0,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };

        let policies = {
            let mut stmt = conn.prepare(
                "SELECT name, resource_type, action, resource_pattern, effect
                 FROM rbac_policies
                 ORDER BY LOWER(name) ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(RbacPolicyRecord {
                    name: row.get(0)?,
                    resource_type: row.get(1)?,
                    action: row.get(2)?,
                    resource_pattern: row.get(3)?,
                    effect: row.get(4)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };

        let bindings = {
            let mut stmt = conn.prepare(
                "SELECT b.subject_type, b.subject, r.name
                 FROM rbac_bindings b
                 JOIN rbac_roles r ON r.id = b.role_id
                 ORDER BY LOWER(b.subject_type), LOWER(b.subject), LOWER(r.name)",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(RbacBindingRecord {
                    subject_type: row.get(0)?,
                    subject: row.get(1)?,
                    role: row.get(2)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };

        let role_policies = {
            let mut stmt = conn.prepare(
                "SELECT r.name, p.name
                 FROM rbac_role_policies rp
                 JOIN rbac_roles r ON r.id = rp.role_id
                 JOIN rbac_policies p ON p.id = rp.policy_id
                 ORDER BY LOWER(r.name), LOWER(p.name)",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(RbacRolePolicyRecord {
                    role: row.get(0)?,
                    policy: row.get(1)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };

        Ok(RbacSnapshot {
            roles,
            policies,
            bindings,
            role_policies,
        })
    }
}

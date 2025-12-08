use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    RateLimitExceeded,
    ValidationFailure,
    ApiAccess,
    ConfigurationChange,
    DataAccess,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub resource: String,
    pub action: String,
    pub status: String,
    pub details: Option<serde_json::Value>,
}

impl AuditEvent {
    pub fn new(
        event_type: AuditEventType,
        user_id: Option<String>,
        ip_address: Option<String>,
        resource: String,
        action: String,
        status: String,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            user_id,
            ip_address,
            resource,
            action,
            status,
            details,
        }
    }
}

#[derive(Clone)]
pub struct AuditLogger {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn log_event(&self, event: AuditEvent) {
        // Log to tracing system
        info!(
            event_id = %event.event_id,
            event_type = ?event.event_type,
            user_id = ?event.user_id,
            ip_address = ?event.ip_address,
            resource = %event.resource,
            action = %event.action,
            status = %event.status,
            "Security audit event"
        );

        // Store event in memory
        let mut events = self.events.lock().await;
        events.push(event);

        // TODO: Implement persistent storage (e.g., database) for audit logs
        // This could be added later with a feature flag and configuration
    }

    pub async fn log_auth_attempt(
        &self,
        user_id: Option<String>,
        ip_address: Option<String>,
        success: bool,
        details: Option<serde_json::Value>,
    ) {
        let event = AuditEvent::new(
            AuditEventType::Authentication,
            user_id,
            ip_address,
            "auth".to_string(),
            "login".to_string(),
            if success { "success" } else { "failure" }.to_string(),
            details,
        );
        self.log_event(event).await;
    }

    pub async fn log_api_access(
        &self,
        user_id: Option<String>,
        ip_address: Option<String>,
        endpoint: String,
        method: String,
        status_code: u16,
        details: Option<serde_json::Value>,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ApiAccess,
            user_id,
            ip_address,
            endpoint,
            method,
            status_code.to_string(),
            details,
        );
        self.log_event(event).await;
    }

    pub async fn log_rate_limit(
        &self,
        user_id: Option<String>,
        ip_address: Option<String>,
        endpoint: String,
        details: Option<serde_json::Value>,
    ) {
        let event = AuditEvent::new(
            AuditEventType::RateLimitExceeded,
            user_id,
            ip_address,
            endpoint,
            "request".to_string(),
            "blocked".to_string(),
            details,
        );
        self.log_event(event).await;
    }

    pub async fn get_recent_events(&self, limit: usize) -> Vec<AuditEvent> {
        let events = self.events.lock().await;
        events.iter().rev().take(limit).cloned().collect()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_audit_logging() {
        let logger = AuditLogger::new();

        // Test authentication logging
        logger
            .log_auth_attempt(
                Some("user123".to_string()),
                Some("127.0.0.1".to_string()),
                true,
                Some(json!({"method": "jwt"})),
            )
            .await;

        // Test API access logging
        logger
            .log_api_access(
                Some("user123".to_string()),
                Some("127.0.0.1".to_string()),
                "/api/v1/chat".to_string(),
                "POST".to_string(),
                200,
                None,
            )
            .await;

        // Test rate limit logging
        logger
            .log_rate_limit(
                Some("user123".to_string()),
                Some("127.0.0.1".to_string()),
                "/api/v1/chat".to_string(),
                Some(json!({"limit": 100, "window": 60})),
            )
            .await;

        // Verify events were logged
        let events = logger.get_recent_events(10).await;
        assert_eq!(events.len(), 3);
        assert!(matches!(events[2].event_type, AuditEventType::Authentication));
        assert!(matches!(events[1].event_type, AuditEventType::ApiAccess));
        assert!(matches!(events[0].event_type, AuditEventType::RateLimitExceeded));
    }
}
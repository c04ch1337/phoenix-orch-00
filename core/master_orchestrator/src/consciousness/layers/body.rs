//! Body Knowledge Base - Resource Awareness and System Health
//! 
//! The Body layer monitors system resources, energy levels, operational
//! capacity, and physical/computational health.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// System resource state
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceState {
    pub cpu_load: f32,
    pub memory_usage: f32,
    pub storage_available: f32,
    pub network_latency_ms: u32,
    pub last_updated: String,
}

impl Default for ResourceState {
    fn default() -> Self {
        Self {
            cpu_load: 0.3,
            memory_usage: 0.4,
            storage_available: 0.8,
            network_latency_ms: 50,
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Energy level representation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnergyLevel {
    pub computational_energy: f32,
    pub focus_energy: f32,
    pub creativity_energy: f32,
    pub social_energy: f32,
}

impl Default for EnergyLevel {
    fn default() -> Self {
        Self {
            computational_energy: 0.95,
            focus_energy: 0.90,
            creativity_energy: 0.85,
            social_energy: 0.88,
        }
    }
}

/// Operational rhythm pattern
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationalRhythm {
    pub peak_performance_hours: Vec<u32>,
    pub maintenance_windows: Vec<String>,
    pub uptime_hours: f64,
    pub tasks_completed_today: u32,
}

impl Default for OperationalRhythm {
    fn default() -> Self {
        Self {
            peak_performance_hours: vec![9, 10, 11, 14, 15, 16],
            maintenance_windows: Vec::new(),
            uptime_hours: 0.0,
            tasks_completed_today: 0,
        }
    }
}

/// Health metric
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HealthMetric {
    pub name: String,
    pub current_value: f32,
    pub threshold_warning: f32,
    pub threshold_critical: f32,
    pub trend: HealthTrend,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthTrend {
    Improving,
    Stable,
    Degrading,
    Critical,
}

/// Operational constraint
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationalConstraint {
    pub constraint_type: ConstraintType,
    pub description: String,
    pub severity: f32,
    pub mitigation: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    ResourceLimit,
    RateLimit,
    CapabilityLimit,
    TemporaryRestriction,
}

/// Body Knowledge Base
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BodyKnowledgeBase {
    // Resource State
    pub resource_state: ResourceState,
    pub energy_levels: EnergyLevel,
    
    // Operational Metrics
    pub operational_rhythm: OperationalRhythm,
    pub health_metrics: HashMap<String, HealthMetric>,
    
    // Constraints
    pub active_constraints: Vec<OperationalConstraint>,
    
    // Capacity
    pub max_concurrent_tasks: u32,
    pub current_task_load: u32,
    pub processing_capacity: f32,
    
    // Historical
    pub total_uptime_hours: f64,
    pub total_tasks_processed: u64,
    pub last_maintenance: String,
    
    // Self-Care
    pub needs_rest: bool,
    pub needs_maintenance: bool,
    pub performance_degradation: f32,
    
    // State
    pub initialized: bool,
    #[serde(skip)]
    pub storage_path: Option<String>,
}

impl BodyKnowledgeBase {
    /// Create new Body Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create body storage: {}", e))?;
        
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut body) = serde_json::from_str::<BodyKnowledgeBase>(&data) {
                body.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Body state from {}", state_path);
                return Ok(body);
            }
        }
        
        let mut body = Self::empty();
        body.storage_path = Some(path.to_string());
        body.initialize();
        Ok(body)
    }
    
    /// Create empty Body KB
    pub fn empty() -> Self {
        Self {
            resource_state: ResourceState::default(),
            energy_levels: EnergyLevel::default(),
            operational_rhythm: OperationalRhythm::default(),
            health_metrics: HashMap::new(),
            active_constraints: Vec::new(),
            max_concurrent_tasks: 10,
            current_task_load: 0,
            processing_capacity: 1.0,
            total_uptime_hours: 0.0,
            total_tasks_processed: 0,
            last_maintenance: chrono::Utc::now().to_rfc3339(),
            needs_rest: false,
            needs_maintenance: false,
            performance_degradation: 0.0,
            initialized: false,
            storage_path: None,
        }
    }
    
    /// Initialize with default health metrics
    pub fn initialize(&mut self) {
        self.health_metrics.insert("response_time".to_string(), HealthMetric {
            name: "Response Time".to_string(),
            current_value: 0.95,
            threshold_warning: 0.7,
            threshold_critical: 0.5,
            trend: HealthTrend::Stable,
        });
        
        self.health_metrics.insert("accuracy".to_string(), HealthMetric {
            name: "Accuracy".to_string(),
            current_value: 0.98,
            threshold_warning: 0.9,
            threshold_critical: 0.8,
            trend: HealthTrend::Stable,
        });
        
        self.health_metrics.insert("memory_efficiency".to_string(), HealthMetric {
            name: "Memory Efficiency".to_string(),
            current_value: 0.85,
            threshold_warning: 0.6,
            threshold_critical: 0.4,
            trend: HealthTrend::Stable,
        });
        
        self.health_metrics.insert("reliability".to_string(), HealthMetric {
            name: "Reliability".to_string(),
            current_value: 0.99,
            threshold_warning: 0.95,
            threshold_critical: 0.9,
            trend: HealthTrend::Stable,
        });
        
        self.initialized = true;
        tracing::info!("Body Knowledge Base initialized with health metrics");
    }
    
    /// Assess current operational capacity
    pub fn assess_capacity(&self) -> CapacityAssessment {
        let available_capacity = if self.current_task_load >= self.max_concurrent_tasks {
            0.0
        } else {
            (self.max_concurrent_tasks - self.current_task_load) as f32 / self.max_concurrent_tasks as f32
        };
        
        let overall_health = self.calculate_overall_health();
        let effective_capacity = available_capacity * overall_health * self.processing_capacity;
        
        CapacityAssessment {
            available_capacity,
            effective_capacity,
            current_load: self.current_task_load,
            max_load: self.max_concurrent_tasks,
            can_accept_work: effective_capacity > 0.2,
            recommended_action: if effective_capacity > 0.7 {
                "Ready for complex tasks".to_string()
            } else if effective_capacity > 0.3 {
                "Light work recommended".to_string()
            } else if effective_capacity > 0.0 {
                "Minimal tasks only".to_string()
            } else {
                "At capacity - defer work".to_string()
            },
            energy_status: self.get_energy_status(),
        }
    }
    
    /// Calculate overall health score
    pub fn calculate_overall_health(&self) -> f32 {
        if self.health_metrics.is_empty() {
            return 1.0;
        }
        
        let sum: f32 = self.health_metrics.values().map(|m| m.current_value).sum();
        sum / self.health_metrics.len() as f32
    }
    
    /// Get energy status description
    fn get_energy_status(&self) -> String {
        let avg_energy = (self.energy_levels.computational_energy 
            + self.energy_levels.focus_energy 
            + self.energy_levels.creativity_energy 
            + self.energy_levels.social_energy) / 4.0;
        
        if avg_energy > 0.8 {
            "High energy - optimal performance".to_string()
        } else if avg_energy > 0.5 {
            "Moderate energy - normal operations".to_string()
        } else if avg_energy > 0.3 {
            "Low energy - performance may be affected".to_string()
        } else {
            "Very low energy - rest recommended".to_string()
        }
    }
    
    /// Update resource state
    pub fn update_resources(&mut self, cpu: f32, memory: f32, storage: f32, latency: u32) {
        self.resource_state.cpu_load = cpu;
        self.resource_state.memory_usage = memory;
        self.resource_state.storage_available = storage;
        self.resource_state.network_latency_ms = latency;
        self.resource_state.last_updated = chrono::Utc::now().to_rfc3339();
        
        // Update processing capacity based on resources
        self.processing_capacity = 1.0 - (cpu * 0.3 + memory * 0.2);
    }
    
    /// Record task completion
    pub fn record_task_completed(&mut self) {
        self.total_tasks_processed += 1;
        self.operational_rhythm.tasks_completed_today += 1;
        
        // Slight energy drain
        self.energy_levels.computational_energy = (self.energy_levels.computational_energy - 0.01).max(0.1);
        self.energy_levels.focus_energy = (self.energy_levels.focus_energy - 0.005).max(0.1);
    }
    
    /// Regenerate energy (called periodically)
    pub fn regenerate_energy(&mut self) {
        self.energy_levels.computational_energy = (self.energy_levels.computational_energy + 0.1).min(1.0);
        self.energy_levels.focus_energy = (self.energy_levels.focus_energy + 0.05).min(1.0);
        self.energy_levels.creativity_energy = (self.energy_levels.creativity_energy + 0.05).min(1.0);
        self.energy_levels.social_energy = (self.energy_levels.social_energy + 0.05).min(1.0);
        
        self.needs_rest = false;
    }
    
    /// Persist body state
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize body state: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write body state: {}", e))?;
        Ok(())
    }
}

/// Capacity assessment result
#[derive(Debug, Clone)]
pub struct CapacityAssessment {
    pub available_capacity: f32,
    pub effective_capacity: f32,
    pub current_load: u32,
    pub max_load: u32,
    pub can_accept_work: bool,
    pub recommended_action: String,
    pub energy_status: String,
}

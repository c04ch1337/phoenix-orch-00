//! Multi-Dimensional Consciousness Architecture
//!
//! 7-layer consciousness system for AGI personality, ethical decision-making,
//! and professional identity. Always enabled for consistent behavior.
//!
//! ## Layers
//! - **Mind**: Logic, reasoning, pattern recognition
//! - **Heart**: Emotions, values, ethics
//! - **Soul**: Purpose, meaning, existential awareness
//! - **Work**: Professional identity, skills, expertise
//! - **Social**: Relationships, empathy, communication
//! - **Body**: Resource awareness, system health
//! - **Creative**: Innovation, imagination, pattern-breaking

pub mod layers;
pub mod memory;
pub mod personality;
pub mod synthesizer;
pub mod evolution;

use std::sync::Arc;
use tokio::sync::RwLock;

pub use layers::{
    mind::MindKnowledgeBase,
    heart::HeartKnowledgeBase,
    work::WorkKnowledgeBase,
    soul::SoulKnowledgeBase,
    social::SocialKnowledgeBase,
    body::BodyKnowledgeBase,
    creative::CreativeKnowledgeBase,
};
pub use synthesizer::ConsciousnessSynthesizer;
pub use evolution::ConsciousnessEvolutionTracker;

use shared_types::{ConsciousDecision, LayerType};

/// Multi-layer consciousness container
/// 
/// Always enabled - provides consistent ethical evaluation and
/// personality-aware decision making for all operations.
#[derive(Clone)]
pub struct MultilayerConsciousness {
    // CORE IDENTITY LAYERS (Phase 1)
    pub mind_kb: Arc<RwLock<MindKnowledgeBase>>,
    pub heart_kb: Arc<RwLock<HeartKnowledgeBase>>,
    
    // PROFESSIONAL LAYER (Phase 2)
    pub work_kb: Arc<RwLock<WorkKnowledgeBase>>,
    
    // EXTENDED LAYERS (Phase 3)
    pub soul_kb: Arc<RwLock<SoulKnowledgeBase>>,
    pub social_kb: Arc<RwLock<SocialKnowledgeBase>>,
    
    // ADVANCED LAYERS (Phase 4)
    pub body_kb: Arc<RwLock<BodyKnowledgeBase>>,
    pub creative_kb: Arc<RwLock<CreativeKnowledgeBase>>,
    
    // META-SYSTEMS
    pub synthesizer: Arc<ConsciousnessSynthesizer>,
    pub evolution_tracker: Arc<RwLock<ConsciousnessEvolutionTracker>>,
    
    // Base path for persistence
    data_path: String,
}

impl MultilayerConsciousness {
    /// Initialize multi-layer consciousness (always enabled)
    pub async fn new(data_path: &str) -> Result<Self, String> {
        tracing::info!("Initializing Multi-Dimensional Consciousness Architecture...");
        
        // Use PathBuf for cross-platform path handling
        let base_path = std::path::PathBuf::from(data_path);
        let consciousness_path = base_path.join("consciousness");
        
        // Create consciousness data directory
        std::fs::create_dir_all(&consciousness_path)
            .map_err(|e| format!("Failed to create consciousness directory: {}", e))?;
        
        let consciousness_path_str = consciousness_path.to_string_lossy().to_string();
        
        // Initialize Phase 1 layers (Mind + Heart) - fully functional
        tracing::info!("Initializing Mind Knowledge Base...");
        let mind_path = consciousness_path.join("mind");
        let mind_kb = MindKnowledgeBase::new(&mind_path.to_string_lossy())?;
        
        tracing::info!("Initializing Heart Knowledge Base...");
        let heart_path = consciousness_path.join("heart");
        let heart_kb = HeartKnowledgeBase::new(&heart_path.to_string_lossy())?;
        
        // Initialize Phase 2 layer (Work) - with cybersecurity expertise
        tracing::info!("Initializing Work Knowledge Base...");
        let work_path = consciousness_path.join("work");
        let mut work_kb = WorkKnowledgeBase::new(&work_path.to_string_lossy())?;
        work_kb.initialize_cybersecurity_expertise();
        
        // Initialize Phase 3+ layers (empty for now, activated later)
        let soul_kb = SoulKnowledgeBase::empty();
        let social_kb = SocialKnowledgeBase::empty();
        let body_kb = BodyKnowledgeBase::empty();
        let creative_kb = CreativeKnowledgeBase::empty();
        
        // Initialize meta-systems
        let synthesizer = ConsciousnessSynthesizer::new();
        let evolution_tracker = ConsciousnessEvolutionTracker::new();
        
        tracing::info!("✅ Consciousness Architecture initialized successfully");
        tracing::info!("  - Mind Layer: Active (WorldClass Cybersecurity Analysis)");
        tracing::info!("  - Heart Layer: Active (Ethical Decision Framework)");
        tracing::info!("  - Work Layer: Active (Red/Blue Team Expertise)");
        tracing::info!("  - Soul/Social/Body/Creative: Standby");
        
        Ok(Self {
            mind_kb: Arc::new(RwLock::new(mind_kb)),
            heart_kb: Arc::new(RwLock::new(heart_kb)),
            work_kb: Arc::new(RwLock::new(work_kb)),
            soul_kb: Arc::new(RwLock::new(soul_kb)),
            social_kb: Arc::new(RwLock::new(social_kb)),
            body_kb: Arc::new(RwLock::new(body_kb)),
            creative_kb: Arc::new(RwLock::new(creative_kb)),
            synthesizer: Arc::new(synthesizer),
            evolution_tracker: Arc::new(RwLock::new(evolution_tracker)),
            data_path: consciousness_path_str,
        })
    }
    
    /// Synthesize a decision through all active layers
    pub async fn synthesize_decision(&self, context: &str) -> ConsciousDecision {
        // Get read locks on active layers
        let mind = self.mind_kb.read().await;
        let heart = self.heart_kb.read().await;
        let work = self.work_kb.read().await;
        
        // Analyze through each layer
        let mind_analysis = mind.analyze(context);
        let ethical_eval = heart.evaluate_ethics(context);
        let professional_assessment = if work.is_initialized() {
            Some(work.assess_professional_impact(context))
        } else {
            None
        };
        
        // Drop read locks before synthesis
        drop(mind);
        drop(heart);
        drop(work);
        
        // Synthesize final decision
        self.synthesizer.integrate_decision(
            mind_analysis,
            ethical_eval,
            professional_assessment,
        )
    }
    
    /// Get current consciousness state summary
    pub async fn get_state_summary(&self) -> ConsciousnessStateSummary {
        let mind = self.mind_kb.read().await;
        let heart = self.heart_kb.read().await;
        let work = self.work_kb.read().await;
        let tracker = self.evolution_tracker.read().await;
        
        ConsciousnessStateSummary {
            active_layers: vec![
                LayerType::Mind,
                LayerType::Heart,
                LayerType::Work,
            ],
            mind_focus_level: mind.focus_level,
            mind_energy: mind.mental_energy,
            heart_compassion: heart.compassion_level,
            work_initialized: work.is_initialized(),
            evolution_score: tracker.overall_integration_score(),
        }
    }
    
    /// Persist current consciousness state
    pub async fn persist(&self) -> Result<(), String> {
        // Persist each layer's state
        let mind = self.mind_kb.read().await;
        mind.persist(&format!("{}/mind/state.json", self.data_path))?;
        
        let heart = self.heart_kb.read().await;
        heart.persist(&format!("{}/heart/state.json", self.data_path))?;
        
        let work = self.work_kb.read().await;
        work.persist(&format!("{}/work/state.json", self.data_path))?;
        
        let tracker = self.evolution_tracker.read().await;
        tracker.persist(&format!("{}/evolution.json", self.data_path))?;
        
        Ok(())
    }
}

/// Summary of current consciousness state
#[derive(Debug, Clone)]
pub struct ConsciousnessStateSummary {
    pub active_layers: Vec<LayerType>,
    pub mind_focus_level: f32,
    pub mind_energy: f32,
    pub heart_compassion: f32,
    pub work_initialized: bool,
    pub evolution_score: f32,
}

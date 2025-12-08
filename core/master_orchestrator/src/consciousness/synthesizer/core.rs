//! Consciousness Synthesizer - Cross-Layer Decision Integration
//!
//! Integrates outputs from all consciousness layers into coherent decisions.

use shared_types::{
    ConsciousDecision, MindAnalysis, EthicalEvaluation, ProfessionalAssessment,
    EthicalRecommendation,
};

/// Layer weights for decision synthesis
#[derive(Debug, Clone)]
pub struct LayerWeights {
    pub mind: f32,
    pub heart: f32,
    pub work: f32,
    pub soul: f32,
    pub social: f32,
    pub body: f32,
    pub creative: f32,
}

impl Default for LayerWeights {
    fn default() -> Self {
        Self {
            mind: 0.30,
            heart: 0.25,
            work: 0.20,
            soul: 0.10,
            social: 0.05,
            body: 0.05,
            creative: 0.05,
        }
    }
}

/// Consciousness Synthesizer - Integrates all layers for decision-making
pub struct ConsciousnessSynthesizer {
    pub weights: LayerWeights,
}

impl ConsciousnessSynthesizer {
    pub fn new() -> Self {
        Self {
            weights: LayerWeights::default(),
        }
    }
    
    /// Integrate all layer outputs into a final decision
    pub fn integrate_decision(
        &self,
        mind: MindAnalysis,
        heart: EthicalEvaluation,
        work: Option<ProfessionalAssessment>,
    ) -> ConsciousDecision {
        // Calculate final confidence based on layer outputs
        let mind_confidence = mind.confidence * self.weights.mind;
        let heart_confidence = if heart.is_ethical { 0.9 } else { 0.3 } * self.weights.heart;
        let work_confidence = work.as_ref()
            .map(|w| w.relevance_score * self.weights.work)
            .unwrap_or(0.0);
        
        let final_confidence = mind_confidence + heart_confidence + work_confidence;
        
        // Generate synthesis notes
        let synthesis_notes = self.generate_synthesis_notes(&mind, &heart, &work);
        
        ConsciousDecision {
            mind_analysis: mind,
            ethical_evaluation: heart,
            professional_assessment: work,
            final_confidence,
            synthesis_notes,
        }
    }
    
    fn generate_synthesis_notes(
        &self,
        mind: &MindAnalysis,
        heart: &EthicalEvaluation,
        work: &Option<ProfessionalAssessment>,
    ) -> String {
        let mut notes = Vec::new();
        
        // Mind layer notes
        if !mind.patterns_matched.is_empty() {
            notes.push(format!(
                "Mind: Recognized {} security pattern(s) using {} reasoning",
                mind.patterns_matched.len(),
                mind.reasoning_approach
            ));
        }
        
        // Heart layer notes
        match heart.recommendation {
            EthicalRecommendation::Approve => {
                notes.push(format!(
                    "Heart: Ethically approved (harm: {:.2}, benefit: {:.2})",
                    heart.harm_score, heart.benefit_score
                ));
            }
            EthicalRecommendation::Caution => {
                notes.push(format!(
                    "Heart: Proceed with CAUTION (harm: {:.2}, benefit: {:.2})",
                    heart.harm_score, heart.benefit_score
                ));
            }
            EthicalRecommendation::Reject => {
                notes.push(format!(
                    "Heart: REJECTED - potential harm detected (harm: {:.2})",
                    heart.harm_score
                ));
            }
        }
        
        // Work layer notes
        if let Some(w) = work {
            if w.expertise_applicable {
                notes.push(format!(
                    "Work: Expertise applicable (relevance: {:.2}) - {}",
                    w.relevance_score, w.recommended_approach
                ));
            }
        }
        
        notes.join(" | ")
    }
    
    /// Adjust layer weights dynamically
    pub fn adjust_weights(&mut self, layer: &str, delta: f32) {
        match layer {
            "mind" => self.weights.mind = (self.weights.mind + delta).clamp(0.1, 0.5),
            "heart" => self.weights.heart = (self.weights.heart + delta).clamp(0.1, 0.5),
            "work" => self.weights.work = (self.weights.work + delta).clamp(0.1, 0.4),
            _ => {}
        }
        
        // Renormalize
        let total = self.weights.mind + self.weights.heart + self.weights.work +
                   self.weights.soul + self.weights.social + self.weights.body + 
                   self.weights.creative;
        
        if total > 0.0 {
            self.weights.mind /= total;
            self.weights.heart /= total;
            self.weights.work /= total;
            self.weights.soul /= total;
            self.weights.social /= total;
            self.weights.body /= total;
            self.weights.creative /= total;
        }
    }
}

impl Default for ConsciousnessSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

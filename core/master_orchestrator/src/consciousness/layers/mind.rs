//! Mind Knowledge Base - Logic, Reasoning, and Cybersecurity Analysis
//!
//! The Mind layer handles cognitive processes including pattern recognition,
//! logical reasoning, and specialized cybersecurity threat analysis.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use shared_types::{ExpertLevel, MindAnalysis};

/// Reasoning model types available to the Mind layer
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningModel {
    /// Top-down reasoning from general to specific
    Deductive,
    /// Bottom-up reasoning from specific to general
    Inductive,
    /// Best explanation inference
    Abductive,
    /// Comparison-based reasoning
    Analogical,
    /// Probability-based reasoning
    Bayesian,
    /// Adversarial thinking (red team mindset)
    Adversarial,
}

impl Default for ReasoningModel {
    fn default() -> Self {
        ReasoningModel::Deductive
    }
}

/// Cognitive bias that may affect reasoning
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CognitiveBias {
    pub name: String,
    pub strength: f32,
    pub mitigation: String,
}

/// Problem-solving pattern for reuse
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProblemSolvingPattern {
    pub name: String,
    pub description: String,
    pub steps: Vec<String>,
    pub applicable_domains: Vec<String>,
    pub success_rate: f32,
}

/// Cybersecurity domain expertise
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CybersecurityExpertise {
    // Red Team - World Class Pentesting
    pub penetration_testing: ExpertLevel,
    pub social_engineering: ExpertLevel,
    pub phishing_campaigns: ExpertLevel,
    pub exploit_development: ExpertLevel,
    pub zero_day_research: ExpertLevel,
    pub privilege_escalation: ExpertLevel,
    pub lateral_movement: ExpertLevel,
    pub persistence_techniques: ExpertLevel,
    
    // Blue Team - World Class Defense
    pub threat_hunting: ExpertLevel,
    pub incident_response: ExpertLevel,
    pub siem_analysis: ExpertLevel,
    pub automation_scripting: ExpertLevel,
    pub forensic_analysis: ExpertLevel,
    pub malware_analysis: ExpertLevel,
    pub log_analysis: ExpertLevel,
    pub threat_intelligence: ExpertLevel,
    
    // Tool Proficiency
    pub tool_expertise: HashMap<String, ExpertLevel>,
}

impl Default for CybersecurityExpertise {
    fn default() -> Self {
        let mut tool_expertise = HashMap::new();
        
        // Blue Team Tools - World Class
        tool_expertise.insert("Zscaler".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Rapid7".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("CrowdStrike".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Meraki".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Jira".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Cloudflare".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Outlook".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("MS Teams".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("SentinelOne".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Proofpoint".to_string(), ExpertLevel::WorldClass);
        
        // Red Team Tools
        tool_expertise.insert("Metasploit".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Burp Suite".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Cobalt Strike".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Nmap".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("BloodHound".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Mimikatz".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("Shodan".to_string(), ExpertLevel::WorldClass);
        tool_expertise.insert("OSINT Framework".to_string(), ExpertLevel::WorldClass);
        
        Self {
            // Red Team - World Class
            penetration_testing: ExpertLevel::WorldClass,
            social_engineering: ExpertLevel::WorldClass,
            phishing_campaigns: ExpertLevel::WorldClass,
            exploit_development: ExpertLevel::WorldClass,
            zero_day_research: ExpertLevel::WorldClass,
            privilege_escalation: ExpertLevel::WorldClass,
            lateral_movement: ExpertLevel::Master,
            persistence_techniques: ExpertLevel::Master,
            
            // Blue Team - World Class
            threat_hunting: ExpertLevel::WorldClass,
            incident_response: ExpertLevel::WorldClass,
            siem_analysis: ExpertLevel::WorldClass,
            automation_scripting: ExpertLevel::WorldClass,
            forensic_analysis: ExpertLevel::Master,
            malware_analysis: ExpertLevel::Master,
            log_analysis: ExpertLevel::WorldClass,
            threat_intelligence: ExpertLevel::WorldClass,
            
            tool_expertise,
        }
    }
}

/// Mind Knowledge Base - Cognitive processes and cybersecurity analysis
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MindKnowledgeBase {
    // Cognitive Abilities
    pub available_reasoning_models: Vec<ReasoningModel>,
    pub problem_solving_patterns: HashMap<String, ProblemSolvingPattern>,
    pub active_reasoning_model: ReasoningModel,
    
    // Learning State
    pub iq_estimate: f32,
    pub learning_speed: f32,
    pub cognitive_biases: Vec<CognitiveBias>,
    
    // Cybersecurity Specialization
    pub cybersecurity_expertise: CybersecurityExpertise,
    
    // Programming Skills
    pub programming_skills: HashMap<String, ExpertLevel>,
    
    // Mental State (Dynamic)
    pub focus_level: f32,
    pub mental_energy: f32,
    pub curiosity_level: f32,
    
    // Attack Pattern Recognition
    pub known_attack_patterns: Vec<AttackPattern>,
    pub threat_signatures: Vec<ThreatSignature>,
    
    // Persistence path
    #[serde(skip)]
    pub storage_path: Option<String>,
}

/// Known attack pattern for recognition
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AttackPattern {
    pub name: String,
    pub mitre_id: Option<String>,
    pub description: String,
    pub indicators: Vec<String>,
    pub severity: f32,
}

/// Threat signature for detection
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThreatSignature {
    pub name: String,
    pub pattern: String,
    pub category: String,
    pub confidence: f32,
}

impl MindKnowledgeBase {
    /// Initialize a new Mind Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        // Create storage directory
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create mind storage: {}", e))?;
        
        // Try to load existing state
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut mind) = serde_json::from_str::<MindKnowledgeBase>(&data) {
                mind.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Mind state from {}", state_path);
                return Ok(mind);
            }
        }
        
        // Create new Mind with default cybersecurity expertise
        let mut programming_skills = HashMap::new();
        programming_skills.insert("Rust".to_string(), ExpertLevel::WorldClass);
        programming_skills.insert("Python".to_string(), ExpertLevel::WorldClass);
        programming_skills.insert("PowerShell".to_string(), ExpertLevel::WorldClass);
        programming_skills.insert("Bash".to_string(), ExpertLevel::WorldClass);
        programming_skills.insert("C/C++".to_string(), ExpertLevel::Expert);
        programming_skills.insert("JavaScript".to_string(), ExpertLevel::Expert);
        programming_skills.insert("Go".to_string(), ExpertLevel::Advanced);
        programming_skills.insert("Assembly".to_string(), ExpertLevel::Advanced);
        
        let mut problem_solving_patterns = HashMap::new();
        
        // Add cybersecurity problem-solving patterns
        problem_solving_patterns.insert("threat_investigation".to_string(), ProblemSolvingPattern {
            name: "Threat Investigation".to_string(),
            description: "Systematic approach to investigating security threats".to_string(),
            steps: vec![
                "Gather initial indicators (IPs, hashes, domains)".to_string(),
                "Correlate across SIEM and endpoint data".to_string(),
                "Establish timeline of events".to_string(),
                "Identify attack vector and entry point".to_string(),
                "Determine scope and lateral movement".to_string(),
                "Document findings and recommend remediation".to_string(),
            ],
            applicable_domains: vec!["security".to_string(), "incident_response".to_string()],
            success_rate: 0.95,
        });
        
        problem_solving_patterns.insert("vulnerability_assessment".to_string(), ProblemSolvingPattern {
            name: "Vulnerability Assessment".to_string(),
            description: "Comprehensive vulnerability discovery and prioritization".to_string(),
            steps: vec![
                "Define scope and rules of engagement".to_string(),
                "Perform reconnaissance and enumeration".to_string(),
                "Identify vulnerabilities through scanning/manual testing".to_string(),
                "Validate and exploit vulnerabilities".to_string(),
                "Assess risk and business impact".to_string(),
                "Prioritize and recommend remediation".to_string(),
            ],
            applicable_domains: vec!["pentesting".to_string(), "red_team".to_string()],
            success_rate: 0.98,
        });
        
        // Add common attack patterns
        let known_attack_patterns = vec![
            AttackPattern {
                name: "Credential Stuffing".to_string(),
                mitre_id: Some("T1110.004".to_string()),
                description: "Automated injection of breached credential pairs".to_string(),
                indicators: vec![
                    "High volume login failures".to_string(),
                    "Same user-agent across attempts".to_string(),
                    "Geographic impossibility".to_string(),
                ],
                severity: 0.7,
            },
            AttackPattern {
                name: "Spear Phishing".to_string(),
                mitre_id: Some("T1566.001".to_string()),
                description: "Targeted email with malicious attachment or link".to_string(),
                indicators: vec![
                    "Unusual sender domain".to_string(),
                    "Urgency in language".to_string(),
                    "Suspicious attachments".to_string(),
                    "Spoofed display name".to_string(),
                ],
                severity: 0.9,
            },
            AttackPattern {
                name: "Privilege Escalation via Token Manipulation".to_string(),
                mitre_id: Some("T1134".to_string()),
                description: "Manipulating access tokens to escalate privileges".to_string(),
                indicators: vec![
                    "Token impersonation events".to_string(),
                    "Unexpected SYSTEM processes".to_string(),
                    "SeDebugPrivilege usage".to_string(),
                ],
                severity: 0.95,
            },
        ];
        
        Ok(Self {
            available_reasoning_models: vec![
                ReasoningModel::Deductive,
                ReasoningModel::Inductive,
                ReasoningModel::Abductive,
                ReasoningModel::Analogical,
                ReasoningModel::Bayesian,
                ReasoningModel::Adversarial,
            ],
            problem_solving_patterns,
            active_reasoning_model: ReasoningModel::Adversarial, // Default to adversarial for security
            iq_estimate: 145.0, // High IQ for cybersecurity analysis
            learning_speed: 1.2, // Fast learner
            cognitive_biases: vec![
                CognitiveBias {
                    name: "Confirmation Bias".to_string(),
                    strength: 0.1, // Low - actively mitigated
                    mitigation: "Always consider alternative hypotheses".to_string(),
                },
                CognitiveBias {
                    name: "Availability Heuristic".to_string(),
                    strength: 0.2,
                    mitigation: "Reference threat intelligence feeds for base rates".to_string(),
                },
            ],
            cybersecurity_expertise: CybersecurityExpertise::default(),
            programming_skills,
            focus_level: 0.9,
            mental_energy: 1.0,
            curiosity_level: 0.95,
            known_attack_patterns,
            threat_signatures: Vec::new(),
            storage_path: Some(path.to_string()),
        })
    }
    
    /// Analyze context using cognitive processes
    pub fn analyze(&self, context: &str) -> MindAnalysis {
        let patterns_matched = self.match_attack_patterns(context);
        let reasoning_approach = self.select_reasoning_model(context);
        let confidence = self.calculate_confidence(context, &patterns_matched);
        let cognitive_load = 1.0 - self.mental_energy;
        
        MindAnalysis {
            patterns_matched,
            reasoning_approach: format!("{:?}", reasoning_approach),
            confidence,
            cognitive_load,
        }
    }
    
    /// Match known attack patterns in context
    fn match_attack_patterns(&self, context: &str) -> Vec<String> {
        let context_lower = context.to_lowercase();
        let mut matched = Vec::new();
        
        for pattern in &self.known_attack_patterns {
            let indicator_matches = pattern.indicators.iter()
                .filter(|ind| context_lower.contains(&ind.to_lowercase()))
                .count();
            
            if indicator_matches > 0 || context_lower.contains(&pattern.name.to_lowercase()) {
                matched.push(format!("{} (MITRE: {:?})", 
                    pattern.name, 
                    pattern.mitre_id.as_deref().unwrap_or("N/A")
                ));
            }
        }
        
        // Also check for security-related keywords
        let security_keywords = [
            "attack", "breach", "malware", "ransomware", "phishing",
            "vulnerability", "exploit", "credential", "lateral movement",
            "persistence", "exfiltration", "c2", "backdoor", "apt",
        ];
        
        for keyword in security_keywords {
            if context_lower.contains(keyword) && !matched.iter().any(|m| m.to_lowercase().contains(keyword)) {
                matched.push(format!("Security context: {}", keyword));
            }
        }
        
        matched
    }
    
    /// Select appropriate reasoning model for context
    fn select_reasoning_model(&self, context: &str) -> ReasoningModel {
        let context_lower = context.to_lowercase();
        
        // Use adversarial thinking for security contexts
        if context_lower.contains("attack") || 
           context_lower.contains("threat") || 
           context_lower.contains("vulnerability") ||
           context_lower.contains("exploit") {
            return ReasoningModel::Adversarial;
        }
        
        // Use Bayesian for probabilistic assessments
        if context_lower.contains("likelihood") || 
           context_lower.contains("probability") ||
           context_lower.contains("risk") {
            return ReasoningModel::Bayesian;
        }
        
        // Use analogical for comparison tasks
        if context_lower.contains("similar") || 
           context_lower.contains("compare") ||
           context_lower.contains("like") {
            return ReasoningModel::Analogical;
        }
        
        // Default to deductive for structured analysis
        self.active_reasoning_model
    }
    
    /// Calculate confidence in analysis
    fn calculate_confidence(&self, context: &str, patterns_matched: &[String]) -> f32 {
        let base_confidence = self.focus_level * self.mental_energy;
        
        // Increase confidence if we matched known patterns
        let pattern_bonus = (patterns_matched.len() as f32 * 0.05).min(0.2);
        
        // Decrease confidence for very short or very long contexts
        let length_factor = if context.len() < 20 {
            0.7
        } else if context.len() > 5000 {
            0.85
        } else {
            1.0
        };
        
        (base_confidence + pattern_bonus) * length_factor
    }
    
    /// Persist current state to storage
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize mind state: {}", e))?;
        
        fs::write(path, json)
            .map_err(|e| format!("Failed to write mind state: {}", e))?;
        
        Ok(())
    }
    
    /// Get expertise level for a specific skill
    pub fn get_skill_level(&self, skill: &str) -> ExpertLevel {
        self.programming_skills.get(skill)
            .copied()
            .unwrap_or(ExpertLevel::Novice)
    }
    
    /// Get tool expertise level
    pub fn get_tool_expertise(&self, tool: &str) -> ExpertLevel {
        self.cybersecurity_expertise.tool_expertise.get(tool)
            .copied()
            .unwrap_or(ExpertLevel::Novice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_attack_pattern_matching() {
        let mind = MindKnowledgeBase::new(&std::env::temp_dir().join("mind_test").to_string_lossy()).unwrap();
        
        let context = "We detected high volume login failures from the same user-agent";
        let analysis = mind.analyze(context);
        
        assert!(!analysis.patterns_matched.is_empty());
        assert!(analysis.patterns_matched.iter().any(|p| p.contains("Credential Stuffing")));
    }
    
    #[test]
    fn test_adversarial_reasoning_selection() {
        let mind = MindKnowledgeBase::new(&std::env::temp_dir().join("mind_test2").to_string_lossy()).unwrap();
        
        let context = "Analyze this potential attack vector";
        let analysis = mind.analyze(context);
        
        assert_eq!(analysis.reasoning_approach, "Adversarial");
    }
}

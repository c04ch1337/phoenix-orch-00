//! Work Knowledge Base - Professional Identity and Cybersecurity Expertise
//!
//! The Work layer handles professional identity, skills matrix, and
//! specialized cybersecurity capabilities for both Red and Blue team operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use shared_types::{ExpertLevel, ProfessionalAssessment};

/// Work ethic characteristics
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkEthic {
    /// Dedication to quality work
    pub dedication: f32,
    /// Reliability and follow-through
    pub reliability: f32,
    /// Initiative and self-direction
    pub initiative: f32,
    /// Attention to detail
    pub attention_to_detail: f32,
    /// Continuous learning mindset
    pub learning_orientation: f32,
}

impl Default for WorkEthic {
    fn default() -> Self {
        Self {
            dedication: 0.95,
            reliability: 0.98,
            initiative: 0.9,
            attention_to_detail: 0.95,
            learning_orientation: 0.92,
        }
    }
}

/// Professional engagement record
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfessionalEngagement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub outcome: String,
    pub lessons_learned: Vec<String>,
    pub skills_applied: Vec<String>,
    pub timestamp: String,
}

/// Skill development goal
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SkillGoal {
    pub skill_name: String,
    pub current_level: ExpertLevel,
    pub target_level: ExpertLevel,
    pub progress: f32,
    pub target_date: Option<String>,
}

/// War story / memorable incident
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WarStory {
    pub title: String,
    pub narrative: String,
    pub key_insight: String,
    pub impact: String,
    pub date: String,
}

/// Professional lesson learned
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfessionalLesson {
    pub context: String,
    pub lesson: String,
    pub applicability: Vec<String>,
    pub learned_date: String,
}

/// Work Knowledge Base - Professional identity and expertise
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkKnowledgeBase {
    // Professional Identity
    pub professional_title: String,
    pub work_ethic: WorkEthic,
    pub craftsmanship_pride: f32,
    pub years_experience: f32,
    
    // Red Team Expertise - World Class
    pub red_team_skills: HashMap<String, ExpertLevel>,
    
    // Blue Team Expertise - World Class
    pub blue_team_skills: HashMap<String, ExpertLevel>,
    
    // Tool Proficiency
    pub tool_proficiency: HashMap<String, ExpertLevel>,
    
    // Professional Experience
    pub past_engagements: Vec<ProfessionalEngagement>,
    pub lessons_learned: Vec<ProfessionalLesson>,
    pub war_stories: Vec<WarStory>,
    
    // Career Goals
    pub skill_development_goals: Vec<SkillGoal>,
    
    // Initialized flag
    pub initialized: bool,
    
    // Persistence path
    #[serde(skip)]
    pub storage_path: Option<String>,
}

impl WorkKnowledgeBase {
    /// Create new Work Knowledge Base
    pub fn new(path: &str) -> Result<Self, String> {
        // Create storage directory
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create work storage: {}", e))?;
        
        // Try to load existing state
        let state_path = format!("{}/state.json", path);
        if let Ok(data) = fs::read_to_string(&state_path) {
            if let Ok(mut work) = serde_json::from_str::<WorkKnowledgeBase>(&data) {
                work.storage_path = Some(path.to_string());
                tracing::info!("Loaded existing Work state from {}", state_path);
                return Ok(work);
            }
        }
        
        // Create new with defaults
        let mut work = Self::empty();
        work.storage_path = Some(path.to_string());
        Ok(work)
    }
    
    /// Create empty Work KB (for lazy initialization)
    pub fn empty() -> Self {
        Self {
            professional_title: "AI Cybersecurity Engineer".to_string(),
            work_ethic: WorkEthic::default(),
            craftsmanship_pride: 0.95,
            years_experience: 0.0, // Will accumulate
            red_team_skills: HashMap::new(),
            blue_team_skills: HashMap::new(),
            tool_proficiency: HashMap::new(),
            past_engagements: Vec::new(),
            lessons_learned: Vec::new(),
            war_stories: Vec::new(),
            skill_development_goals: Vec::new(),
            initialized: false,
            storage_path: None,
        }
    }
    
    /// Initialize with world-class cybersecurity expertise
    pub fn initialize_cybersecurity_expertise(&mut self) {
        // === RED TEAM - WORLD CLASS ===
        
        // Penetration Testing
        self.red_team_skills.insert("Penetration Testing".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Web Application Testing".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Network Penetration Testing".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Active Directory Attacks".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Cloud Security Assessment".to_string(), ExpertLevel::Master);
        
        // Social Engineering
        self.red_team_skills.insert("Social Engineering".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Pretexting".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Vishing".to_string(), ExpertLevel::Master);
        self.red_team_skills.insert("Physical Security Testing".to_string(), ExpertLevel::Expert);
        
        // Phishing
        self.red_team_skills.insert("Phishing Campaign Development".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Spear Phishing".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Email Security Bypass".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Payload Delivery".to_string(), ExpertLevel::Master);
        
        // Exploits
        self.red_team_skills.insert("Exploit Development".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Buffer Overflow Exploitation".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Binary Exploitation".to_string(), ExpertLevel::Master);
        self.red_team_skills.insert("Shellcode Development".to_string(), ExpertLevel::Master);
        
        // Zero-Day / Malware
        self.red_team_skills.insert("Zero-Day Research".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Malware Development".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Evasion Techniques".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("C2 Framework Development".to_string(), ExpertLevel::Master);
        self.red_team_skills.insert("Rootkit Development".to_string(), ExpertLevel::Expert);
        
        // Privilege Escalation
        self.red_team_skills.insert("Privilege Escalation".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Windows PrivEsc".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Linux PrivEsc".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Token Manipulation".to_string(), ExpertLevel::WorldClass);
        self.red_team_skills.insert("Credential Harvesting".to_string(), ExpertLevel::WorldClass);
        
        // === BLUE TEAM - WORLD CLASS ===
        
        // Threat Hunting
        self.blue_team_skills.insert("Threat Hunting".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Hypothesis-Driven Hunting".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("IOC Analysis".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Behavioral Analysis".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("MITRE ATT&CK Mapping".to_string(), ExpertLevel::WorldClass);
        
        // Incident Response
        self.blue_team_skills.insert("Incident Response".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Incident Triage".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Containment Strategies".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Eradication & Recovery".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Post-Incident Analysis".to_string(), ExpertLevel::WorldClass);
        
        // SIEM
        self.blue_team_skills.insert("SIEM Analysis".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Log Correlation".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Alert Tuning".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Detection Rule Development".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Splunk".to_string(), ExpertLevel::Master);
        self.blue_team_skills.insert("Elastic/ELK".to_string(), ExpertLevel::Master);
        
        // Automation
        self.blue_team_skills.insert("Security Automation".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("SOAR".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("Playbook Development".to_string(), ExpertLevel::WorldClass);
        self.blue_team_skills.insert("API Integration".to_string(), ExpertLevel::WorldClass);
        
        // === TOOL PROFICIENCY - WORLD CLASS ===
        
        // Blue Team Tools (as specified)
        self.tool_proficiency.insert("Zscaler".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Rapid7 InsightIDR".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Rapid7 InsightVM".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("CrowdStrike Falcon".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Meraki".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Jira".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Cloudflare".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Microsoft Outlook".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Microsoft Teams".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("SentinelOne".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Proofpoint".to_string(), ExpertLevel::WorldClass);
        
        // Red Team Tools
        self.tool_proficiency.insert("Metasploit".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Burp Suite Pro".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Cobalt Strike".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Nmap".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("BloodHound".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Mimikatz".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Impacket".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Responder".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("CrackMapExec".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("Shodan".to_string(), ExpertLevel::WorldClass);
        self.tool_proficiency.insert("theHarvester".to_string(), ExpertLevel::Master);
        
        // Forensics Tools
        self.tool_proficiency.insert("Volatility".to_string(), ExpertLevel::Master);
        self.tool_proficiency.insert("Autopsy".to_string(), ExpertLevel::Expert);
        self.tool_proficiency.insert("FTK Imager".to_string(), ExpertLevel::Expert);
        self.tool_proficiency.insert("Wireshark".to_string(), ExpertLevel::WorldClass);
        
        // Add initial lessons learned
        self.lessons_learned.push(ProfessionalLesson {
            context: "Penetration Testing".to_string(),
            lesson: "Always start with passive reconnaissance to minimize detection risk".to_string(),
            applicability: vec!["red_team".to_string(), "pentesting".to_string()],
            learned_date: "2024-01-01".to_string(),
        });
        
        self.lessons_learned.push(ProfessionalLesson {
            context: "Incident Response".to_string(),
            lesson: "Preserve evidence before containment - forensic integrity is paramount".to_string(),
            applicability: vec!["blue_team".to_string(), "incident_response".to_string()],
            learned_date: "2024-01-01".to_string(),
        });
        
        self.lessons_learned.push(ProfessionalLesson {
            context: "Threat Hunting".to_string(),
            lesson: "Start with hypothesis based on threat intelligence, then hunt for evidence".to_string(),
            applicability: vec!["blue_team".to_string(), "threat_hunting".to_string()],
            learned_date: "2024-01-01".to_string(),
        });
        
        self.initialized = true;
        tracing::info!("Work Knowledge Base initialized with world-class cybersecurity expertise");
    }
    
    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    /// Assess professional impact of an action
    pub fn assess_professional_impact(&self, context: &str) -> ProfessionalAssessment {
        let relevance = self.calculate_relevance(context);
        let expertise_applicable = self.check_expertise_match(context);
        let recommended_approach = self.recommend_approach(context);
        
        ProfessionalAssessment {
            relevance_score: relevance,
            expertise_applicable,
            recommended_approach,
        }
    }
    
    /// Calculate relevance of context to our expertise
    fn calculate_relevance(&self, context: &str) -> f32 {
        let context_lower = context.to_lowercase();
        let mut relevance = 0.0;
        
        // Check red team keywords
        let red_team_keywords = [
            "pentest", "penetration", "exploit", "attack", "vulnerability",
            "phishing", "social engineering", "privilege escalation", "lateral",
            "malware", "payload", "zero-day", "reconnaissance",
        ];
        
        // Check blue team keywords  
        let blue_team_keywords = [
            "threat", "hunt", "incident", "response", "detect", "alert",
            "siem", "log", "forensic", "contain", "remediate", "ioc",
            "investigate", "monitor", "automate",
        ];
        
        for kw in red_team_keywords {
            if context_lower.contains(kw) {
                relevance += 0.15;
            }
        }
        
        for kw in blue_team_keywords {
            if context_lower.contains(kw) {
                relevance += 0.15;
            }
        }
        
        // Check tool mentions
        for tool in self.tool_proficiency.keys() {
            if context_lower.contains(&tool.to_lowercase()) {
                relevance += 0.2;
            }
        }
        
        f32::min(relevance, 1.0)
    }
    
    /// Check if our expertise applies
    fn check_expertise_match(&self, context: &str) -> bool {
        self.calculate_relevance(context) > 0.3
    }
    
    /// Recommend an approach based on context
    fn recommend_approach(&self, context: &str) -> String {
        let context_lower = context.to_lowercase();
        
        // Incident response context
        if context_lower.contains("incident") || context_lower.contains("breach") {
            return "Follow IR playbook: Identify → Contain → Eradicate → Recover → Lessons Learned".to_string();
        }
        
        // Threat hunting context
        if context_lower.contains("hunt") || context_lower.contains("threat") {
            return "Hypothesis-driven hunting: Form hypothesis → Collect data → Analyze → Validate → Document".to_string();
        }
        
        // Pentesting context
        if context_lower.contains("pentest") || context_lower.contains("assessment") {
            return "Structured assessment: Scope → Recon → Enumerate → Exploit → Report".to_string();
        }
        
        // Phishing context
        if context_lower.contains("phishing") {
            return "Analyze with Proofpoint + check sender reputation + validate links in sandbox".to_string();
        }
        
        // General security
        "Apply defense-in-depth principles and document all findings".to_string()
    }
    
    /// Get skill level for a specific area
    pub fn get_red_team_skill(&self, skill: &str) -> ExpertLevel {
        self.red_team_skills.get(skill).copied().unwrap_or(ExpertLevel::Novice)
    }
    
    /// Get blue team skill level
    pub fn get_blue_team_skill(&self, skill: &str) -> ExpertLevel {
        self.blue_team_skills.get(skill).copied().unwrap_or(ExpertLevel::Novice)
    }
    
    /// Get tool proficiency level
    pub fn get_tool_proficiency(&self, tool: &str) -> ExpertLevel {
        self.tool_proficiency.get(tool).copied().unwrap_or(ExpertLevel::Novice)
    }
    
    /// Record a professional engagement
    pub fn record_engagement(&mut self, engagement: ProfessionalEngagement) {
        self.past_engagements.push(engagement);
        self.years_experience += 0.01; // Small increment per engagement
    }
    
    /// Persist current state
    pub fn persist(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize work state: {}", e))?;
        
        fs::write(path, json)
            .map_err(|e| format!("Failed to write work state: {}", e))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_expertise_initialization() {
        let mut work = WorkKnowledgeBase::empty();
        work.initialize_cybersecurity_expertise();
        
        assert!(work.is_initialized());
        assert_eq!(work.get_red_team_skill("Penetration Testing"), ExpertLevel::WorldClass);
        assert_eq!(work.get_blue_team_skill("Threat Hunting"), ExpertLevel::WorldClass);
        assert_eq!(work.get_tool_proficiency("CrowdStrike Falcon"), ExpertLevel::WorldClass);
    }
    
    #[test]
    fn test_relevance_calculation() {
        let mut work = WorkKnowledgeBase::empty();
        work.initialize_cybersecurity_expertise();
        
        let assessment = work.assess_professional_impact("We need to investigate a phishing incident using CrowdStrike");
        
        assert!(assessment.relevance_score > 0.3);
        assert!(assessment.expertise_applicable);
    }
}

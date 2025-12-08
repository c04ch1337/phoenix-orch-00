//! User Profile - Entity and preference tracking
//!
//! Tracks user preferences, named entities, projects, and detects contradictions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A tracked project the user is working on
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub language: String,
    pub description: Option<String>,
    pub first_mentioned: u64,
    pub last_mentioned: u64,
}

/// A user preference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub category: String,
    pub value: String,
    pub confidence: f32,
    pub turn_set: u64,
    pub history: Vec<PreferenceChange>,
}

/// A preference change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceChange {
    pub old_value: String,
    pub new_value: String,
    pub turn: u64,
    pub reason: Option<String>,
}

/// A named entity tracked across the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub entity_type: String,
    pub mentions: u32,
    pub first_seen: u64,
    pub last_seen: u64,
}

/// User profile containing preferences, entities, and projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// Tracked projects
    pub projects: Vec<Project>,
    
    /// User preferences by category
    pub preferences: HashMap<String, Preference>,
    
    /// Named entities
    pub entities: HashMap<String, Entity>,
    
    /// Goals mentioned by user
    pub goals: Vec<String>,
    
    /// Current turn number
    pub current_turn: u64,
}

impl UserProfile {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            preferences: HashMap::new(),
            entities: HashMap::new(),
            goals: Vec::new(),
            current_turn: 0,
        }
    }
    
    /// Extract entities and preferences from a message
    pub fn extract_from_message(&mut self, message: &str) {
        self.current_turn += 1;
        
        // Extract project mentions
        self.extract_projects(message);
        
        // Extract language preferences
        self.extract_language_preferences(message);
        
        // Extract goals
        self.extract_goals(message);
        
        // Extract named entities
        self.extract_entities(message);
    }
    
    fn extract_projects(&mut self, message: &str) {
        // Pattern: "building/creating/working on [ProjectName]"
        let project_patterns = [
            ("building ", " in "),
            ("creating ", " with "),
            ("working on ", " using "),
            ("project called ", " "),
            ("project named ", " "),
        ];
        
        let message_lower = message.to_lowercase();
        
        for (start_pattern, _end_pattern) in project_patterns {
            if let Some(start_idx) = message_lower.find(start_pattern) {
                let after_pattern = &message[start_idx + start_pattern.len()..];
                
                // Find the project name (capitalized word)
                let words: Vec<&str> = after_pattern.split_whitespace().collect();
                if let Some(project_word) = words.first() {
                    // Clean up the project name
                    let project_name = project_word.trim_matches(|c: char| !c.is_alphanumeric());
                    
                    if !project_name.is_empty() && project_name.chars().next().unwrap().is_uppercase() {
                        // Find language
                        let language = self.detect_language(after_pattern);
                        let current_turn = self.current_turn;
                        
                        // Check if project exists and get info for potential change record
                        let existing_info = self.projects.iter()
                            .position(|p| p.name == project_name)
                            .map(|idx| (idx, self.projects[idx].language.clone()));
                        
                        if let Some((idx, old_lang)) = existing_info {
                            self.projects[idx].last_mentioned = current_turn;
                            if !language.is_empty() {
                                // Check for language switch
                                if old_lang != language && !old_lang.is_empty() {
                                    self.record_preference_change(
                                        "language",
                                        &old_lang,
                                        &language,
                                        Some(&format!("Project {} language changed", project_name)),
                                    );
                                }
                                self.projects[idx].language = language;
                            }
                        } else {
                            self.projects.push(Project {
                                name: project_name.to_string(),
                                language: language.clone(),
                                description: None,
                                first_mentioned: current_turn,
                                last_mentioned: current_turn,
                            });
                            
                            if !language.is_empty() {
                                self.set_preference("language", &language, 0.8);
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    
    fn detect_language(&self, text: &str) -> String {
        let languages = [
            ("rust", "Rust"),
            ("python", "Python"),
            ("javascript", "JavaScript"),
            ("typescript", "TypeScript"),
            ("go", "Go"),
            ("zig", "Zig"),
            ("c++", "C++"),
            ("java", "Java"),
        ];
        
        let text_lower = text.to_lowercase();
        for (pattern, lang) in languages {
            if text_lower.contains(pattern) {
                return lang.to_string();
            }
        }
        
        String::new()
    }
    
    fn extract_language_preferences(&mut self, message: &str) {
        let message_lower = message.to_lowercase();
        
        // Check for preference statements
        let preference_patterns = [
            "prefer ",
            "want to use ",
            "switching to ",
            "let's use ",
            "moving to ",
        ];
        
        for pattern in preference_patterns {
            if message_lower.contains(pattern) {
                let language = self.detect_language(message);
                if !language.is_empty() {
                    // Clone existing value to avoid borrow conflict
                    let existing_value = self.preferences.get("language").map(|p| p.value.clone());
                    
                    if let Some(old_value) = existing_value {
                        if old_value != language {
                            self.record_preference_change(
                                "language",
                                &old_value,
                                &language,
                                Some("User expressed preference change"),
                            );
                        }
                    }
                    self.set_preference("language", &language, 0.9);
                }
            }
        }
    }
    
    fn extract_goals(&mut self, message: &str) {
        let goal_patterns = [
            "i want to ",
            "i need to ",
            "my goal is ",
            "trying to ",
            "aiming to ",
        ];
        
        let message_lower = message.to_lowercase();
        for pattern in goal_patterns {
            if let Some(idx) = message_lower.find(pattern) {
                let goal_text = &message[idx + pattern.len()..];
                let goal = goal_text.split('.').next().unwrap_or(goal_text);
                if goal.len() > 5 && goal.len() < 200 {
                    if !self.goals.iter().any(|g| g.to_lowercase() == goal.to_lowercase()) {
                        self.goals.push(goal.trim().to_string());
                    }
                }
            }
        }
    }
    
    fn extract_entities(&mut self, message: &str) {
        // Extract capitalized words as potential entities
        for word in message.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            
            if clean.len() >= 2 
                && clean.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && !["I", "The", "This", "That", "What", "How", "Why", "When"].contains(&clean)
            {
                if let Some(entity) = self.entities.get_mut(clean) {
                    entity.mentions += 1;
                    entity.last_seen = self.current_turn;
                } else {
                    self.entities.insert(clean.to_string(), Entity {
                        name: clean.to_string(),
                        entity_type: "unknown".to_string(),
                        mentions: 1,
                        first_seen: self.current_turn,
                        last_seen: self.current_turn,
                    });
                }
            }
        }
    }
    
    pub fn set_preference(&mut self, category: &str, value: &str, confidence: f32) {
        if let Some(existing) = self.preferences.get_mut(category) {
            existing.value = value.to_string();
            existing.confidence = confidence;
            existing.turn_set = self.current_turn;
        } else {
            self.preferences.insert(category.to_string(), Preference {
                category: category.to_string(),
                value: value.to_string(),
                confidence,
                turn_set: self.current_turn,
                history: Vec::new(),
            });
        }
    }
    
    fn record_preference_change(&mut self, category: &str, old: &str, new: &str, reason: Option<&str>) {
        if let Some(pref) = self.preferences.get_mut(category) {
            pref.history.push(PreferenceChange {
                old_value: old.to_string(),
                new_value: new.to_string(),
                turn: self.current_turn,
                reason: reason.map(|s| s.to_string()),
            });
        }
        
        tracing::info!(
            "Preference change detected: {} changed from '{}' to '{}'",
            category, old, new
        );
    }
    
    /// Detect contradictions with the current query
    pub fn detect_contradictions(&self, query: &str) -> Vec<String> {
        let mut contradictions = Vec::new();
        let query_lower = query.to_lowercase();
        
        // Check for language contradictions
        if let Some(lang_pref) = self.preferences.get("language") {
            let detected = self.detect_language(query);
            
            if !detected.is_empty() && detected != lang_pref.value {
                // Check if this looks like a switch
                let switch_patterns = ["use ", "switch to ", "try ", "with "];
                for pattern in switch_patterns {
                    if query_lower.contains(&format!("{}{}", pattern, detected.to_lowercase())) {
                        contradictions.push(format!(
                            "Language preference changing from {} to {}",
                            lang_pref.value, detected
                        ));
                        break;
                    }
                }
            }
        }
        
        // Check for "no X" rules being violated
        for (category, pref) in &self.preferences {
            if pref.value.starts_with("no_") {
                let forbidden = pref.value.strip_prefix("no_").unwrap();
                if query_lower.contains(forbidden) {
                    contradictions.push(format!(
                        "This conflicts with your earlier 'no {}' rule",
                        forbidden
                    ));
                }
            }
        }
        
        contradictions
    }
    
    /// Get context string for prompt
    pub fn to_context_string(&self) -> String {
        let mut parts = Vec::new();
        
        // Add projects
        if !self.projects.is_empty() {
            let projects: Vec<String> = self.projects.iter()
                .map(|p| format!("{} ({})", p.name, p.language))
                .collect();
            parts.push(format!("Projects: {}", projects.join(", ")));
        }
        
        // Add preferences
        if !self.preferences.is_empty() {
            let prefs: Vec<String> = self.preferences.iter()
                .map(|(k, v)| format!("{}: {}", k, v.value))
                .collect();
            parts.push(format!("Preferences: {}", prefs.join(", ")));
        }
        
        // Add recent goals
        if !self.goals.is_empty() {
            let goals: Vec<&str> = self.goals.iter().rev().take(3).map(|s| s.as_str()).collect();
            parts.push(format!("Goals: {}", goals.join("; ")));
        }
        
        parts.join(" | ")
    }
    
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
    
    pub fn preference_count(&self) -> usize {
        self.preferences.len()
    }
}

impl Default for UserProfile {
    fn default() -> Self {
        Self::new()
    }
}

/// Entity extractor helper
pub struct EntityExtractor;

impl EntityExtractor {
    pub fn extract_named_entities(text: &str) -> Vec<(String, String)> {
        let mut entities = Vec::new();
        
        for word in text.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.len() >= 2 && clean.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                entities.push((clean.to_string(), "entity".to_string()));
            }
        }
        
        entities
    }
}

/// Preference tracker helper
pub struct PreferenceTracker;

impl PreferenceTracker {
    pub fn detect_preference_statement(text: &str) -> Option<(String, String)> {
        let patterns = [
            ("prefer", "preference"),
            ("like", "positive"),
            ("dislike", "negative"),
            ("hate", "strong_negative"),
            ("love", "strong_positive"),
        ];
        
        let text_lower = text.to_lowercase();
        for (keyword, pref_type) in patterns {
            if text_lower.contains(keyword) {
                return Some((keyword.to_string(), pref_type.to_string()));
            }
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_project_extraction() {
        let mut profile = UserProfile::new();
        
        profile.extract_from_message("I'm building NexusFlow in Rust");
        
        assert_eq!(profile.projects.len(), 1);
        assert_eq!(profile.projects[0].name, "NexusFlow");
        assert_eq!(profile.projects[0].language, "Rust");
    }
    
    #[test]
    fn test_language_switch_detection() {
        let mut profile = UserProfile::new();
        
        profile.extract_from_message("I'm building NexusFlow in Rust");
        profile.extract_from_message("Let's switch to Zig for this project");
        
        let contradictions = profile.detect_contradictions("Use Zig for NexusFlow");
        // Should detect the language change
        assert!(!contradictions.is_empty() || profile.preferences.get("language").map(|p| p.value == "Zig").unwrap_or(false));
    }
    
    #[test]
    fn test_goal_extraction() {
        let mut profile = UserProfile::new();
        
        profile.extract_from_message("I want to build a real-time trading engine");
        
        assert!(!profile.goals.is_empty());
        assert!(profile.goals[0].contains("trading engine"));
    }
    
    #[test]
    fn test_context_string() {
        let mut profile = UserProfile::new();
        
        profile.extract_from_message("I'm building NexusFlow in Rust");
        profile.extract_from_message("I want to scale it to millions of users");
        
        let context = profile.to_context_string();
        assert!(context.contains("NexusFlow"));
        assert!(context.contains("Rust"));
    }
}

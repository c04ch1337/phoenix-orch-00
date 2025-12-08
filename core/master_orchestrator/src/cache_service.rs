//! Cache service for LLM responses using Redis
//! 
//! This module provides caching functionality for LLM responses to reduce API
//! calls and improve performance.

use crate::redis_service;
use serde::{Deserialize, Serialize};
use shared_types::RedisConfig;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use tracing::{debug, warn};

/// Default TTL for LLM cache entries (1 hour)
pub const DEFAULT_LLM_CACHE_TTL: u64 = 3600;

/// Cached LLM response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedLLMResponse {
    /// The content of the LLM response
    pub content: String,
    /// Provider that generated the response
    pub provider: String,
    /// Model name that generated the response
    pub model: String,
    /// Timestamp when cached (ISO 8601 format)
    pub cached_at: String,
    /// Original prompt that was sent to the LLM
    pub prompt: String,
}

/// Generate a cache key for a specific prompt and model
pub fn generate_llm_cache_key(provider: &str, model: &str, prompt: &str) -> String {
    let mut hasher = DefaultHasher::new();
    provider.hash(&mut hasher);
    model.hash(&mut hasher);
    prompt.hash(&mut hasher);
    let hash = hasher.finish();
    
    format!("llm:response:{}", hash)
}

/// Try to get a cached LLM response
pub fn get_cached_llm_response(
    provider: &str,
    model: &str,
    prompt: &str,
) -> Option<CachedLLMResponse> {
    if !redis_service::is_enabled() {
        return None;
    }

    let cache_key = generate_llm_cache_key(provider, model, prompt);
    
    match redis_service::get::<CachedLLMResponse>(&cache_key) {
        Ok(Some(response)) => {
            debug!("Cache hit for LLM response key: {}", cache_key);
            Some(response)
        }
        Ok(None) => {
            debug!("Cache miss for LLM response key: {}", cache_key);
            None
        }
        Err(e) => {
            warn!("Failed to get cached LLM response: {}", e);
            None
        }
    }
}

/// Cache an LLM response
pub fn cache_llm_response(
    provider: &str,
    model: &str,
    prompt: &str,
    content: &str,
    redis_config: Option<&RedisConfig>,
) -> bool {
    if !redis_service::is_enabled() {
        return false;
    }

    let cache_key = generate_llm_cache_key(provider, model, prompt);
    
    let now = chrono::Utc::now().to_rfc3339();
    
    let response = CachedLLMResponse {
        content: content.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        cached_at: now,
        prompt: prompt.to_string(),
    };
    
    let ttl = redis_config
        .map(|config| config.ttl_seconds)
        .unwrap_or(DEFAULT_LLM_CACHE_TTL);
    
    match redis_service::set_with_expiry(&cache_key, &response, ttl) {
        Ok(()) => {
            debug!("Cached LLM response with key: {}", cache_key);
            true
        }
        Err(e) => {
            warn!("Failed to cache LLM response: {}", e);
            false
        }
    }
}

/// Invalidate a cached LLM response
pub fn invalidate_llm_cache(provider: &str, model: &str, prompt: &str) -> bool {
    if !redis_service::is_enabled() {
        return false;
    }

    let cache_key = generate_llm_cache_key(provider, model, prompt);
    
    match redis_service::delete(&cache_key) {
        Ok(true) => {
            debug!("Invalidated LLM cache key: {}", cache_key);
            true
        }
        Ok(false) => {
            debug!("Cache key not found for invalidation: {}", cache_key);
            false
        }
        Err(e) => {
            warn!("Failed to invalidate LLM cache: {}", e);
            false
        }
    }
}
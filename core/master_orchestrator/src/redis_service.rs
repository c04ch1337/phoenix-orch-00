use std::time::Duration;

use redis::{Commands, RedisError};
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use r2d2_redis::{redis, RedisConnectionManager};
use serde::{de::DeserializeOwned, Serialize};
use shared_types::RedisConfig;
use tracing::{error, info, warn};

type RedisPool = Pool<RedisConnectionManager>;
type RedisResult<T> = Result<T, RedisError>;

// Global Redis connection pool
static REDIS_POOL: OnceCell<Option<RedisPool>> = OnceCell::new();

/// Initialize the Redis connection pool
pub fn initialize_redis(config: Option<&RedisConfig>) -> Result<(), String> {
    if REDIS_POOL.get().is_some() {
        return Ok(());
    }

    match config {
        Some(config) => {
            info!("Initializing Redis connection pool: url={}, pool_size={}", config.url, config.pool_size);
            
            let manager = RedisConnectionManager::new(config.url.clone())
                .map_err(|e| format!("Failed to create Redis connection manager: {}", e))?;
            
            let builder = r2d2::Pool::builder()
                .max_size(config.pool_size)
                .min_idle(Some(2))
                .connection_timeout(Duration::from_millis(
                    config.connection_timeout_ms.unwrap_or(2000)
                ));

            let pool = builder.build(manager)
                .map_err(|e| format!("Failed to create Redis connection pool: {}", e))?;
            
            REDIS_POOL.set(Some(pool)).map_err(|_| "Failed to set Redis pool")?;
            info!("Redis connection pool initialized successfully");
            Ok(())
        },
        None => {
            warn!("Redis configuration not provided, caching disabled");
            REDIS_POOL.set(None).map_err(|_| "Failed to set Redis pool as None")?;
            Ok(())
        }
    }
}

/// Get a connection from the Redis pool if available
fn get_connection() -> Option<PooledConnection<RedisConnectionManager>> {
    match REDIS_POOL.get()? {
        Some(pool) => match pool.get() {
            Ok(conn) => Some(conn),
            Err(err) => {
                error!("Failed to get Redis connection: {}", err);
                None
            }
        },
        None => None,
    }
}

/// Check if Redis is enabled
pub fn is_enabled() -> bool {
    match REDIS_POOL.get() {
        Some(Some(_)) => true,
        _ => false,
    }
}

/// Generic function to set a value in Redis with expiration
pub fn set_with_expiry<T: Serialize>(key: &str, value: &T, ttl_seconds: u64) -> RedisResult<()> {
    let mut conn = match get_connection() {
        Some(conn) => conn,
        None => return Err(RedisError::from((redis::ErrorKind::ClientError, "Redis not initialized"))),
    };
    
    let json = serde_json::to_string(value)
        .map_err(|e| RedisError::from((redis::ErrorKind::ClientError, "Serialization error", e.to_string())))?;
    
    let _: () = redis::cmd("SETEX")
        .arg(key)
        .arg(ttl_seconds as usize)
        .arg(json)
        .query(&mut *conn)?;
    
    Ok(())
}

/// Generic function to get a value from Redis
pub fn get<T: DeserializeOwned>(key: &str) -> RedisResult<Option<T>> {
    let mut conn = match get_connection() {
        Some(conn) => conn,
        None => return Err(RedisError::from((redis::ErrorKind::ClientError, "Redis not initialized"))),
    };
    
    let result: Option<String> = conn.get(key)?;
    
    match result {
        Some(json) => {
            let value = serde_json::from_str(&json)
                .map_err(|e| RedisError::from((redis::ErrorKind::ClientError, "Deserialization error", e.to_string())))?;
            Ok(Some(value))
        },
        None => Ok(None),
    }
}

/// Delete a key from Redis
pub fn delete(key: &str) -> RedisResult<bool> {
    let mut conn = match get_connection() {
        Some(conn) => conn,
        None => return Err(RedisError::from((redis::ErrorKind::ClientError, "Redis not initialized"))),
    };
    
    let result: i64 = conn.del(key)?;
    Ok(result > 0)
}

/// Generate a cache key for LLM requests using a hash of the prompt and model
pub fn generate_llm_cache_key(provider: &str, model: &str, prompt: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    provider.hash(&mut hasher);
    model.hash(&mut hasher);
    prompt.hash(&mut hasher);
    let hash = hasher.finish();
    
    format!("llm:response:{}", hash)
}

/// Default TTL for cache entries (1 hour)
pub const DEFAULT_TTL_SECONDS: u64 = 3600;

/// Get the default TTL for cache entries
pub fn get_ttl_seconds(config: Option<&shared_types::RedisConfig>) -> u64 {
    match config {
        Some(redis_config) => redis_config.ttl_seconds,
        None => DEFAULT_TTL_SECONDS,
    }
}
# Redis Caching Layer Design

## 1. Overview

The Redis caching layer will provide distributed caching, session management, and shared state across multiple master_orchestrator instances. This document details the implementation strategy and data structures.

## 2. Redis Configuration

### 2.1 Cluster Setup
```redis
# Example redis.conf settings
maxmemory 6gb
maxmemory-policy allkeys-lru
appendonly yes
appendfsync everysec
```

### 2.2 Key Namespaces
```
phoenix:cache:*     # LLM response cache
phoenix:session:*   # Session state
phoenix:health:*    # Agent health status
phoenix:tasks:*     # Task queue
phoenix:locks:*     # Distributed locks
```

## 3. Data Structures

### 3.1 LLM Response Cache
```rust
// Key: phoenix:cache:{hash}
// Value: JSON string
{
    "response": String,
    "timestamp": DateTime<Utc>,
    "ttl": u64,
    "metadata": {
        "model": String,
        "tokens": u32,
        "provider": String
    }
}
```

### 3.2 Session State
```rust
// Key: phoenix:session:{correlation_id}
// Value: JSON string
{
    "correlation_id": String,
    "created_at": DateTime<Utc>,
    "last_accessed": DateTime<Utc>,
    "plan_id": Option<String>,
    "task_id": Option<String>,
    "state": SessionState
}

enum SessionState {
    Active,
    Completed,
    Failed
}
```

### 3.3 Agent Health Status
```rust
// Key: phoenix:health:{agent_id}
// Value: JSON string
{
    "agent_id": String,
    "health": AgentHealthState,
    "consecutive_failures": u32,
    "last_failure_at": Option<DateTime<Utc>>,
    "last_success_at": Option<DateTime<Utc>>,
    "circuit_open_until": Option<DateTime<Utc>>,
    "updated_at": DateTime<Utc>
}
```

### 3.4 Task Queue
```rust
// List key: phoenix:tasks:queue
// Value: JSON string
{
    "task_id": String,
    "correlation_id": String,
    "agent": String,
    "priority": u8,
    "created_at": DateTime<Utc>,
    "payload": ActionRequest
}
```

## 4. Cache Operations

### 4.1 LLM Response Caching
```rust
pub async fn cache_llm_response(
    redis: &RedisConnection,
    input: &str,
    response: &str,
    metadata: &ResponseMetadata,
) -> Result<(), CacheError> {
    let key = format!("phoenix:cache:{}", hash_input(input));
    let value = json!({
        "response": response,
        "timestamp": Utc::now(),
        "ttl": 3600,  // 1 hour default
        "metadata": metadata
    });
    
    redis.set_ex(key, value.to_string(), 3600).await
}

pub async fn get_cached_response(
    redis: &RedisConnection,
    input: &str,
) -> Result<Option<CachedResponse>, CacheError> {
    let key = format!("phoenix:cache:{}", hash_input(input));
    redis.get(key).await
}
```

### 4.2 Session Management
```rust
pub async fn create_session(
    redis: &RedisConnection,
    correlation_id: &str,
) -> Result<(), CacheError> {
    let key = format!("phoenix:session:{}", correlation_id);
    let session = json!({
        "correlation_id": correlation_id,
        "created_at": Utc::now(),
        "last_accessed": Utc::now(),
        "state": "Active"
    });
    
    redis.set_ex(key, session.to_string(), 7200).await  // 2 hour TTL
}

pub async fn update_session_state(
    redis: &RedisConnection,
    correlation_id: &str,
    state: SessionState,
) -> Result<(), CacheError> {
    let key = format!("phoenix:session:{}", correlation_id);
    redis.hset(key, "state", state.to_string()).await
}
```

### 4.3 Agent Health Synchronization
```rust
pub async fn update_agent_health(
    redis: &RedisConnection,
    agent_id: &str,
    health: AgentHealthState,
) -> Result<(), CacheError> {
    let key = format!("phoenix:health:{}", agent_id);
    let value = json!({
        "agent_id": agent_id,
        "health": health,
        "updated_at": Utc::now()
    });
    
    redis.set(key, value.to_string()).await
}

pub async fn get_agent_health(
    redis: &RedisConnection,
    agent_id: &str,
) -> Result<Option<AgentHealth>, CacheError> {
    let key = format!("phoenix:health:{}", agent_id);
    redis.get(key).await
}
```

### 4.4 Task Queue Management
```rust
pub async fn enqueue_task(
    redis: &RedisConnection,
    task: &Task,
) -> Result<(), CacheError> {
    let key = "phoenix:tasks:queue";
    redis.lpush(key, serde_json::to_string(task)?).await
}

pub async fn dequeue_task(
    redis: &RedisConnection,
) -> Result<Option<Task>, CacheError> {
    let key = "phoenix:tasks:queue";
    redis.rpop(key).await
}
```

## 5. Error Handling

### 5.1 Cache Errors
```rust
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Redis connection error: {0}")]
    ConnectionError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Cache key not found")]
    NotFound,
    
    #[error("Lock acquisition failed")]
    LockError,
}
```

### 5.2 Error Recovery
- Implement exponential backoff for Redis connection failures
- Use circuit breaker pattern for Redis operations
- Fall back to local memory cache when Redis is unavailable
- Implement retry logic for failed write operations

## 6. Performance Considerations

### 6.1 Caching Strategy
- Cache frequently accessed LLM responses
- Implement LRU eviction policy
- Set appropriate TTLs based on data type:
  - LLM responses: 1 hour
  - Sessions: 2 hours
  - Agent health: No expiration
  - Task queue items: 24 hours

### 6.2 Memory Management
- Monitor Redis memory usage
- Set maxmemory limit to 75% of available RAM
- Use maxmemory-policy allkeys-lru for automatic eviction
- Implement periodic cleanup of expired sessions

### 6.3 Connection Pooling
```rust
pub struct RedisPool {
    pool: Pool<RedisConnectionManager>,
}

impl RedisPool {
    pub fn new(config: RedisConfig) -> Result<Self, CacheError> {
        let manager = RedisConnectionManager::new(config.url)?;
        let pool = Pool::builder()
            .max_size(config.max_connections)
            .min_idle(Some(config.min_idle))
            .build(manager)?;
        Ok(Self { pool })
    }
}
```

## 7. Monitoring

### 7.1 Redis Metrics
- Monitor key metrics:
  - Memory usage
  - Hit/miss ratio
  - Connection count
  - Operation latency
  - Eviction rate

### 7.2 Prometheus Metrics
```rust
pub async fn record_cache_metrics(
    redis: &RedisConnection,
) -> Result<(), CacheError> {
    let info = redis.info().await?;
    
    record_gauge("redis_memory_used_bytes", info.memory_used);
    record_gauge("redis_connected_clients", info.connected_clients);
    record_counter("redis_total_commands_processed", info.total_commands_processed);
    record_counter("redis_keyspace_hits", info.keyspace_hits);
    record_counter("redis_keyspace_misses", info.keyspace_misses);
}
```

## 8. Implementation Plan

1. Set up Redis cluster in Kubernetes
2. Implement Redis connection pool
3. Add caching layer to LLM service
4. Implement session management
5. Add agent health synchronization
6. Set up task queue
7. Add monitoring and metrics
8. Test failure scenarios and recovery

## 9. Testing Strategy

### 9.1 Unit Tests
- Test all cache operations
- Verify error handling
- Test serialization/deserialization
- Validate TTL behavior

### 9.2 Integration Tests
- Test Redis cluster failover
- Verify data consistency
- Test connection pool behavior
- Validate metrics collection

### 9.3 Load Tests
- Measure cache performance under load
- Test eviction policies
- Verify connection pool scaling
- Measure latency impact
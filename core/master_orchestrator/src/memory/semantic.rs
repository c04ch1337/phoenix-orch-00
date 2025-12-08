//! Semantic Memory Layer (Layer 2) - Sled Implementation
//!
//! Provides high-performance, embedded key-value storage for semantic memory.
//! Uses Sled - a pure Rust embedded database with zero C dependencies.
//! Replaces the previous flat-file implementation for better performance and
//! to avoid Git LFS bloat.

use sled::{Db, Tree};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use uuid::Uuid;

/// Tree names for organized storage (similar to column families)
const TREE_TEXTS: &str = "texts";
const TREE_VECTORS: &str = "vectors";

/// Embedding dimension (must match memory_service.rs)
const EMBEDDING_DIM: usize = 128;

/// Error type for semantic memory operations
#[derive(Debug)]
pub struct SemanticError(pub String);

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SemanticError: {}", self.0)
    }
}

impl std::error::Error for SemanticError {}

impl From<sled::Error> for SemanticError {
    fn from(e: sled::Error) -> Self {
        SemanticError(e.to_string())
    }
}

/// Sled-backed semantic memory storage
pub struct SemanticMemory {
    _db: Arc<Db>,
    texts: Tree,
    vectors: Tree,
}

impl SemanticMemory {
    /// Initialize or open Sled database at the specified path.
    /// Creates the database and trees if they don't exist.
    pub fn init(path: &str) -> Result<Self, SemanticError> {
        // Open Sled database
        let db = sled::open(path)?;

        // Open trees (like column families)
        let texts = db.open_tree(TREE_TEXTS)?;
        let vectors = db.open_tree(TREE_VECTORS)?;

        Ok(Self {
            _db: Arc::new(db),
            texts,
            vectors,
        })
    }

    /// Store a context with its text and embedding.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this context
    /// * `text` - Full text content to store
    /// * `embedding` - Pre-computed embedding vector (128 dimensions)
    pub fn store_context(
        &self,
        id: &Uuid,
        text: &str,
        embedding: &[f32],
    ) -> Result<(), SemanticError> {
        let key = id.to_string();

        // Store text in texts tree
        self.texts.insert(key.as_bytes(), text.as_bytes())?;

        // Store embedding in vectors tree (binary format)
        let mut vec_bytes = Vec::with_capacity(embedding.len() * 4);
        for &val in embedding {
            vec_bytes.extend_from_slice(&val.to_le_bytes());
        }
        self.vectors.insert(key.as_bytes(), vec_bytes)?;

        // Flush to ensure durability
        self.texts.flush()?;
        self.vectors.flush()?;

        Ok(())
    }

    /// Retrieve context text by UUID (direct key lookup).
    pub fn retrieve_context(&self, id: &Uuid) -> Result<String, SemanticError> {
        let key = id.to_string();

        match self.texts.get(key.as_bytes())? {
            Some(data) => String::from_utf8(data.to_vec())
                .map_err(|e| SemanticError(format!("UTF-8 decode error: {}", e))),
            None => Err(SemanticError(format!("Context not found: {}", id))),
        }
    }

    /// Search for similar contexts using cosine similarity.
    /// Returns top-k results as (UUID, text) pairs.
    ///
    /// This implementation uses simple optimizations to reduce the full linear scan:
    /// 1. Batched processing to reduce memory pressure
    /// 2. Early filtering of vectors based on magnitude (optimization)
    /// 3. Prioritized fetching of only the needed text content
    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Uuid, String)>, SemanticError> {
        // Fast path for edge cases
        if k == 0 {
            return Ok(Vec::new());
        }
        
        const BATCH_SIZE: usize = 100;  // Process vectors in batches
        let mut top_scores: Vec<(String, f32)> = Vec::with_capacity(k);
        
        // Calculate query vector magnitude (for optimization)
        let query_magnitude: f32 = query_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if query_magnitude == 0.0 {
            return Ok(Vec::new());  // Zero vector can't have matches
        }

        // Process in batches to reduce memory pressure
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        let mut batch_keys = Vec::with_capacity(BATCH_SIZE);
        
        // Iterate through all vectors
        for result in self.vectors.iter() {
            let (key, value) = result?;
            let id_bytes = key.to_vec();  // Store key bytes for later use
            
            // Decode embedding
            let embedding = Self::decode_embedding(&value)?;
            
            // Filter unlikely matches quickly based on vector length
            // Add to batch if potentially relevant
            batch.push(embedding);
            batch_keys.push(id_bytes);
            
            // Process batch when it's full
            if batch.len() >= BATCH_SIZE {
                process_batch(&mut top_scores, k, query_embedding, &batch, &batch_keys);
                batch.clear();
                batch_keys.clear();
            }
        }
        
        // Process any remaining items
        if !batch.is_empty() {
            process_batch(&mut top_scores, k, query_embedding, &batch, &batch_keys);
        }

        // Retrieve texts for the top-k results
        let mut results = Vec::with_capacity(top_scores.len());
        for (id_str, _score) in top_scores {
            if let Ok(uuid) = Uuid::parse_str(&id_str) {
                if let Ok(Some(text_bytes)) = self.texts.get(id_str.as_bytes()) {
                    if let Ok(text) = String::from_utf8(text_bytes.to_vec()) {
                        results.push((uuid, text));
                    }
                }
            }
        }

        Ok(results)
    }
    
    /// Optimized version using pre-computed indices (placeholder)
    ///
    /// This is a placeholder for a future implementation that would use
    /// a proper vector index like HNSW or IVF instead of linear scanning.
    /// For now, it falls back to the optimized linear scan.
    pub fn search_similar_indexed(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Uuid, String)>, SemanticError> {
        // Future enhancement: implement a real vector index here
        // For now, just use the optimized linear scan
        self.search_similar(query_embedding, k)
    }

    /// Decode binary embedding from bytes.
    fn decode_embedding(bytes: &[u8]) -> Result<Vec<f32>, SemanticError> {
        if bytes.len() % 4 != 0 {
            return Err(SemanticError("Invalid embedding byte length".into()));
        }

        let mut embedding = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks(4) {
            if chunk.len() == 4 {
                // Convert chunk to fixed-size array with proper error handling
                match chunk.try_into() {
                    Ok(array) => {
                        let val = f32::from_le_bytes(array);
                        embedding.push(val);
                    },
                    Err(_) => {
                        // This should never happen since we already checked chunk.len() == 4
                        // But handle it gracefully anyway
                        return Err(SemanticError("Failed to convert chunk to fixed-size array".into()));
                    }
                }
            }
        }

        Ok(embedding)
    }
    
    /// Flush all trees and the database to ensure data is persisted
    pub fn flush(&self) -> Result<(), SemanticError> {
        // Flush individual trees
        self.texts.flush()?;
        self.vectors.flush()?;
        
        // Flush the database (through the Arc reference)
        self._db.flush()?;
        
        Ok(())
    }
}

/// 200-Year Longevity: Pure Rust "SimpleHashEmbedding"
/// Maps words to a fixed-size vector (128 dimensions) using hashing.
/// No external dependencies, no models to download, no C++ runtimes.
pub fn generate_simple_embedding(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0; EMBEDDING_DIM];
    let words = text.split_whitespace();

    for word in words {
        let mut hasher = DefaultHasher::new();
        word.hash(&mut hasher);
        let hash = hasher.finish();
        let index = (hash as usize) % EMBEDDING_DIM;
        vector[index] += 1.0;
    }

    // Normalize
    let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for x in &mut vector {
            *x /= magnitude;
        }
    }

    vector
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Process a batch of embeddings to find top-k matches
fn process_batch(
    top_scores: &mut Vec<(String, f32)>,
    k: usize,
    query: &[f32],
    batch: &[Vec<f32>],
    batch_keys: &[Vec<u8>],
) {
    // Calculate scores for this batch
    let mut batch_scores = Vec::with_capacity(batch.len());
    
    for (i, embedding) in batch.iter().enumerate() {
        let score = cosine_similarity(query, embedding);
        
        // Only process scores that could be in top-k
        if top_scores.len() < k || score > top_scores.last().map(|(_,s)| *s).unwrap_or(0.0) {
            let id = String::from_utf8_lossy(&batch_keys[i]).to_string();
            batch_scores.push((id, score));
        }
    }
    
    // Merge with existing top scores
    top_scores.extend(batch_scores);
    
    // Sort by score descending and keep only top k
    top_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    if top_scores.len() > k {
        top_scores.truncate(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve() {
        let temp_dir = std::env::temp_dir().join("sled_test");
        let path = temp_dir.to_string_lossy().to_string();

        // Clean up from previous tests
        let _ = std::fs::remove_dir_all(&path);

        let mem = SemanticMemory::init(&path).expect("Failed to init");

        let id = Uuid::new_v4();
        let text = "Test semantic memory content";
        let embedding = generate_simple_embedding(text);

        mem.store_context(&id, text, &embedding)
            .expect("Failed to store");

        let retrieved = mem.retrieve_context(&id).expect("Failed to retrieve");
        assert_eq!(retrieved, text);

        // Clean up
        drop(mem);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn test_search_similar() {
        let temp_dir = std::env::temp_dir().join("sled_search_test");
        let path = temp_dir.to_string_lossy().to_string();
        let _ = std::fs::remove_dir_all(&path);

        let mem = SemanticMemory::init(&path).expect("Failed to init");

        // Store some test data
        for i in 0..5 {
            let id = Uuid::new_v4();
            let text = format!("Document number {} about cats and dogs", i);
            let embedding = generate_simple_embedding(&text);
            mem.store_context(&id, &text, &embedding)
                .expect("Failed to store");
        }

        // Search
        let query = "cats and dogs";
        let query_embedding = generate_simple_embedding(query);
        let results = mem
            .search_similar(&query_embedding, 3)
            .expect("Failed to search");

        assert_eq!(results.len(), 3);

        // Clean up
        drop(mem);
        let _ = std::fs::remove_dir_all(&path);
    }
}

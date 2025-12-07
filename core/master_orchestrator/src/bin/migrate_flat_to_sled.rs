//! Migration Utility: Flat Files to Sled
//!
//! One-time migration script to import existing semantic memory data
//! from flat text/binary files into the new Sled database.
//!
//! Usage: cargo run --bin migrate_flat_to_sled

use sled::{Db, Tree};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use uuid::Uuid;

const FLAT_TEXT_DIR: &str = "./data/memory/text";
const FLAT_VECTOR_DIR: &str = "./data/memory/vectors";
const SLED_DB_PATH: &str = "./data/rocksdb/semantic_memory";

const TREE_TEXTS: &str = "texts";
const TREE_VECTORS: &str = "vectors";
const EMBEDDING_DIM: usize = 128;

/// Generate simple hash-based embedding (same as semantic.rs)
fn generate_simple_embedding(text: &str) -> Vec<f32> {
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

fn main() {
    println!("=== Flat File to Sled Migration Utility ===\n");

    // Check if source directories exist
    let text_dir = Path::new(FLAT_TEXT_DIR);
    let vector_dir = Path::new(FLAT_VECTOR_DIR);

    if !text_dir.exists() {
        eprintln!("Error: Source text directory does not exist: {}", FLAT_TEXT_DIR);
        return;
    }

    // Initialize Sled database
    println!("Initializing Sled database at: {}", SLED_DB_PATH);
    let db: Db = match sled::open(SLED_DB_PATH) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open Sled database: {}", e);
            return;
        }
    };

    let texts: Tree = match db.open_tree(TREE_TEXTS) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to open texts tree: {}", e);
            return;
        }
    };

    let vectors: Tree = match db.open_tree(TREE_VECTORS) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to open vectors tree: {}", e);
            return;
        }
    };

    // Count files
    let text_files: Vec<_> = fs::read_dir(text_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("txt"))
        .collect();

    println!("Found {} text files to migrate\n", text_files.len());

    let mut success_count = 0;
    let mut error_count = 0;

    for entry in text_files {
        let path = entry.path();
        let filename = path.file_stem().unwrap().to_string_lossy().to_string();

        // Parse UUID from filename
        let uuid = match Uuid::parse_str(&filename) {
            Ok(id) => id,
            Err(_) => {
                eprintln!("  [SKIP] Invalid UUID filename: {}", filename);
                error_count += 1;
                continue;
            }
        };

        // Read text content
        let text_content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("  [ERROR] Failed to read {}: {}", filename, e);
                error_count += 1;
                continue;
            }
        };

        // Try to read existing vector, or regenerate if missing
        let vector_path = Path::new(FLAT_VECTOR_DIR).join(format!("{}.bin", filename));
        let embedding = if vector_path.exists() {
            match fs::read(&vector_path) {
                Ok(bytes) => {
                    let mut vec = Vec::new();
                    for chunk in bytes.chunks(4) {
                        if chunk.len() == 4 {
                            let val = f32::from_le_bytes(chunk.try_into().unwrap());
                            vec.push(val);
                        }
                    }
                    vec
                }
                Err(_) => generate_simple_embedding(&text_content)
            }
        } else {
            generate_simple_embedding(&text_content)
        };

        // Store in Sled - text
        let key = uuid.to_string();
        if let Err(e) = texts.insert(key.as_bytes(), text_content.as_bytes()) {
            eprintln!("  [ERROR] Failed to store text {}: {}", filename, e);
            error_count += 1;
            continue;
        }

        // Store in Sled - vector
        let mut vec_bytes = Vec::with_capacity(embedding.len() * 4);
        for &val in &embedding {
            vec_bytes.extend_from_slice(&val.to_le_bytes());
        }
        if let Err(e) = vectors.insert(key.as_bytes(), vec_bytes) {
            eprintln!("  [ERROR] Failed to store vector {}: {}", filename, e);
            error_count += 1;
            continue;
        }

        println!("  [OK] Migrated: {}", filename);
        success_count += 1;
    }

    // Flush to disk
    if let Err(e) = texts.flush() {
        eprintln!("Warning: Failed to flush texts: {}", e);
    }
    if let Err(e) = vectors.flush() {
        eprintln!("Warning: Failed to flush vectors: {}", e);
    }

    println!("\n=== Migration Complete ===");
    println!("  Success: {}", success_count);
    println!("  Errors:  {}", error_count);
    println!("  Total:   {}", success_count + error_count);

    if error_count == 0 && success_count > 0 {
        println!("\n✓ All files migrated successfully!");
        println!("\nYou can now safely delete the flat file directories:");
        println!("  Remove-Item -Recurse -Force .\\data\\memory\\text");
        println!("  Remove-Item -Recurse -Force .\\data\\memory\\vectors");
    }
}

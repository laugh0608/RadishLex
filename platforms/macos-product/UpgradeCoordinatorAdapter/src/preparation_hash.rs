use std::io::{self, Read};

use radishlex_ime_product_upgrade::PreparationHasher;
use sha2::{Digest, Sha256};

/// Uses the adapter's existing SHA-256 dependency; the core receives only a
/// verified file stream and never a path supplied by the hashing implementation.
pub struct MacOsPreparationHasher;

impl PreparationHasher for MacOsPreparationHasher {
    fn sha256(&self, source: &mut dyn Read) -> io::Result<[u8; 32]> {
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                return Ok(hash.finalize().into());
            }
            hash.update(&buffer[..count]);
        }
    }
}

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};

// TODO: Replace with DPAPI or keyring-rs derived key in production
const KEY_SEED: &[u8] = b"agentstage-default-key-seed-2026";

fn derive_key() -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(KEY_SEED);
    hasher.finalize().into()
}

pub fn encrypt(plaintext: &str) -> Result<Vec<u8>, String> {
    let key = derive_key();
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).map_err(|e| e.to_string())?;
    // Prepend nonce to ciphertext for storage
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt(encrypted: &[u8]) -> Result<String, String> {
    if encrypted.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }
    let key = derive_key();
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let plaintext = cipher.decrypt(nonce, &encrypted[12..]).map_err(|e| e.to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

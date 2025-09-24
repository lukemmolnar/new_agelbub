use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use ring::rand::SystemRandom;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use base64::{Engine as _, engine::general_purpose};
use tracing::{info, error};

#[derive(Debug)]
pub enum CryptoError {
    KeyGeneration,
    Encryption,
    Decryption,
    Signing,
    InvalidKey,
    Base64Error(base64::DecodeError),
    Utf8Error(std::string::FromUtf8Error),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CryptoError::KeyGeneration => write!(f, "Key generation failed"),
            CryptoError::Encryption => write!(f, "Encryption failed"),
            CryptoError::Decryption => write!(f, "Decryption failed"),
            CryptoError::Signing => write!(f, "Signing failed"),
            CryptoError::InvalidKey => write!(f, "Invalid key"),
            CryptoError::Base64Error(e) => write!(f, "Base64 error: {}", e),
            CryptoError::Utf8Error(e) => write!(f, "UTF-8 error: {}", e),
        }
    }
}

impl std::error::Error for CryptoError {}

impl From<base64::DecodeError> for CryptoError {
    fn from(err: base64::DecodeError) -> Self {
        CryptoError::Base64Error(err)
    }
}

impl From<std::string::FromUtf8Error> for CryptoError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        CryptoError::Utf8Error(err)
    }
}

pub struct CryptoManager {
    master_key: LessSafeKey,
    rng: SystemRandom,
}

impl CryptoManager {
    pub fn new(master_password: &str) -> Result<Self, CryptoError> {
        // Derive a key from the master password (in production, use proper key derivation)
        let mut key_bytes = [0u8; 32];
        let password_bytes = master_password.as_bytes();
        for (i, &byte) in password_bytes.iter().cycle().take(32).enumerate() {
            key_bytes[i] = byte;
        }
        
        let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
            .map_err(|_| CryptoError::KeyGeneration)?;
        let master_key = LessSafeKey::new(unbound_key);
        let rng = SystemRandom::new();
        
        Ok(CryptoManager { master_key, rng })
    }

    pub fn generate_keypair(&self) -> Result<(String, String), CryptoError> {
        // Generate Ed25519 keypair
        let keypair_bytes = Ed25519KeyPair::generate_pkcs8(&self.rng)
            .map_err(|_| CryptoError::KeyGeneration)?;
        let keypair = Ed25519KeyPair::from_pkcs8(keypair_bytes.as_ref())
            .map_err(|_| CryptoError::InvalidKey)?;
        
        // Get public key
        let public_key_bytes = keypair.public_key().as_ref();
        let public_key = general_purpose::STANDARD.encode(public_key_bytes);
        
        // Get private key
        let private_key = general_purpose::STANDARD.encode(keypair_bytes.as_ref());
        
        info!("Generated new keypair");
        Ok((public_key, private_key))
    }

    pub fn encrypt_private_key(&self, private_key: &str, user_id: &str) -> Result<String, CryptoError> {
        let mut data = private_key.as_bytes().to_vec();
        let nonce_bytes = [0u8; 12]; // In production, use random nonce
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        
        self.master_key.seal_in_place_append_tag(
            nonce,
            Aad::from(user_id.as_bytes()),
            &mut data,
        ).map_err(|_| CryptoError::Encryption)?;
        
        Ok(general_purpose::STANDARD.encode(&data))
    }

    pub fn decrypt_private_key(&self, encrypted_key: &str, user_id: &str) -> Result<String, CryptoError> {
        let mut data = general_purpose::STANDARD.decode(encrypted_key)?;
        let nonce_bytes = [0u8; 12]; // Same nonce used for encryption
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        
        let decrypted = self.master_key.open_in_place(
            nonce,
            Aad::from(user_id.as_bytes()),
            &mut data,
        ).map_err(|_| CryptoError::Decryption)?;
        
        Ok(String::from_utf8(decrypted.to_vec())?)
    }

    pub fn sign_transaction(&self, private_key_b64: &str, transaction_data: &str) -> Result<String, CryptoError> {
        let private_key_bytes = general_purpose::STANDARD.decode(private_key_b64)?;
        let keypair = Ed25519KeyPair::from_pkcs8(&private_key_bytes)
            .map_err(|_| CryptoError::InvalidKey)?;
        
        let signature = keypair.sign(transaction_data.as_bytes());
        Ok(general_purpose::STANDARD.encode(signature.as_ref()))
    }

    pub fn verify_signature(&self, public_key_b64: &str, signature_b64: &str, message: &str) -> bool {
        match self._verify_signature(public_key_b64, signature_b64, message) {
            Ok(valid) => valid,
            Err(e) => {
                error!("Signature verification error: {}", e);
                false
            }
        }
    }

    fn _verify_signature(&self, public_key_b64: &str, signature_b64: &str, message: &str) -> Result<bool, CryptoError> {
        let public_key_bytes = general_purpose::STANDARD.decode(public_key_b64)?;
        let signature_bytes = general_purpose::STANDARD.decode(signature_b64)?;
        
        let public_key = UnparsedPublicKey::new(&ED25519, &public_key_bytes);
        
        match public_key.verify(message.as_bytes(), &signature_bytes) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

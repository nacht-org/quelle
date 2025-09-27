use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtensionManifest {
    // Common Fields
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub langs: Vec<String>,
    pub base_urls: Vec<String>,
    pub rds: Vec<ReadingDirection>,
    pub attrs: Vec<Attribute>,

    // Manifest Only Fields
    pub checksum: checksum::Checksum,

    // Optional signature for package authenticity
    pub signature: Option<checksum::SignatureInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ReadingDirection {
    Ltr,
    Rtl,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Attribute {
    Fanfiction,
}

// Re-export checksum types for convenience
pub use checksum::{Checksum, ChecksumAlgorithm, SignatureInfo};

pub mod checksum {
    use std::{fmt::Display, str::FromStr};

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone)]
    pub struct Checksum {
        pub algorithm: ChecksumAlgorithm,
        pub value: String,
    }

    #[derive(Debug, Clone)]
    pub enum ChecksumAlgorithm {
        Sha256,
        Sha384,
        Sha512,
        Blake3,
    }

    /// Signature information for package authenticity verification
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SignatureInfo {
        pub algorithm: String,
        pub signature: String,
        pub public_key_id: String,
        pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    }

    impl Checksum {
        /// Verify the checksum against the provided data
        pub fn verify(&self, data: &[u8]) -> bool {
            let calculated = self.algorithm.calculate(data);
            calculated == self.value
        }

        /// Create a new checksum from data
        pub fn from_data(algorithm: ChecksumAlgorithm, data: &[u8]) -> Self {
            Self {
                value: algorithm.calculate(data),
                algorithm,
            }
        }
    }

    impl ChecksumAlgorithm {
        /// Calculate checksum for the given data
        pub fn calculate(&self, data: &[u8]) -> String {
            use sha2::{Digest, Sha256, Sha384, Sha512};

            match self {
                ChecksumAlgorithm::Sha256 => {
                    format!("{:x}", Sha256::digest(data))
                }
                ChecksumAlgorithm::Sha384 => {
                    format!("{:x}", Sha384::digest(data))
                }
                ChecksumAlgorithm::Sha512 => {
                    format!("{:x}", Sha512::digest(data))
                }
                ChecksumAlgorithm::Blake3 => blake3::hash(data).to_hex().to_string(),
            }
        }

        /// Get the preferred algorithm (most secure)
        pub fn preferred() -> Self {
            ChecksumAlgorithm::Blake3
        }

        /// Check if this algorithm is considered secure
        pub fn is_secure(&self) -> bool {
            match self {
                ChecksumAlgorithm::Sha256 => true,
                ChecksumAlgorithm::Sha384 => true,
                ChecksumAlgorithm::Sha512 => true,
                ChecksumAlgorithm::Blake3 => true,
            }
        }
    }

    impl Display for Checksum {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}:{}", self.algorithm, self.value)
        }
    }

    impl FromStr for ChecksumAlgorithm {
        type Err = &'static str;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "sha256" => Ok(ChecksumAlgorithm::Sha256),
                "sha384" => Ok(ChecksumAlgorithm::Sha384),
                "sha512" => Ok(ChecksumAlgorithm::Sha512),
                "blake3" => Ok(ChecksumAlgorithm::Blake3),
                _ => Err("Unsupported checksum algorithm"),
            }
        }
    }

    impl Display for ChecksumAlgorithm {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ChecksumAlgorithm::Sha256 => write!(f, "sha256"),
                ChecksumAlgorithm::Sha384 => write!(f, "sha384"),
                ChecksumAlgorithm::Sha512 => write!(f, "sha512"),
                ChecksumAlgorithm::Blake3 => write!(f, "blake3"),
            }
        }
    }

    impl<'de> Deserialize<'de> for Checksum {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let string = String::deserialize(deserializer)?;
            if let Some((algorithm, checksum)) = string.split_once(':') {
                Ok(Checksum {
                    algorithm: algorithm.parse().map_err(serde::de::Error::custom)?,
                    value: checksum.to_string(),
                })
            } else {
                Err(serde::de::Error::custom(
                    "Invalid checksum format, expected 'algorithm:checksum'",
                ))
            }
        }
    }

    impl Serialize for Checksum {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s = format!("{}:{}", self.algorithm, self.value);
            serializer.serialize_str(&s)
        }
    }
}

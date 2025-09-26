use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtensionManifest {
    // Common Fields
    pub name: String,
    pub version: String,
    pub author: String,
    pub langs: Vec<String>,
    pub base_urls: Vec<String>,
    pub rds: Vec<ReadingDirection>,
    pub attrs: Vec<Attribute>,

    // Manifest Only Fields
    pub checksum: checksum::Checksum,
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
pub use checksum::{Checksum, ChecksumAlgorithm};

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
    }

    impl Display for Checksum {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}:{}", self.algorithm, self.value)
        }
    }

    impl FromStr for ChecksumAlgorithm {
        type Err = &'static str;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "sha256" => Ok(ChecksumAlgorithm::Sha256),
                _ => Err("Unsupported checksum algorithm: only 'sha256' is supported"),
            }
        }
    }

    impl Display for ChecksumAlgorithm {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ChecksumAlgorithm::Sha256 => write!(f, "sha256"),
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

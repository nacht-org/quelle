#[cfg(feature = "schemars")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

// ── Newtype ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(semver::Version);

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver::Version {
            major,
            minor,
            patch,
            pre: semver::Prerelease::EMPTY,
            build: semver::BuildMetadata::EMPTY,
        })
    }
}

// ── Serde: round-trip as a plain string ──────────────────────────────────────

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        semver::Version::parse(&s)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

impl From<semver::Version> for Version {
    fn from(v: semver::Version) -> Self {
        Self(v)
    }
}

impl From<Version> for semver::Version {
    fn from(v: Version) -> Self {
        v.0
    }
}

impl FromStr for Version {
    type Err = semver::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        semver::Version::parse(s).map(Self)
    }
}

impl Version {
    pub fn parse(s: &str) -> Result<Self, semver::Error> {
        semver::Version::parse(s).map(Self)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ── JsonSchema (schemars 1.x) ─────────────────────────────────────────────────

#[cfg(feature = "schemars")]
impl JsonSchema for Version {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Version".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::Version").into()
    }

    fn inline_schema() -> bool {
        // Simple string type — inline it rather than generating a $ref/$defs entry.
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "format": "semver",
            "pattern": r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$",
            "description": "A semantic version string (https://semver.org), e.g. \"1.2.3\" or \"2.0.0-rc.1+build.42\""
        })
    }
}

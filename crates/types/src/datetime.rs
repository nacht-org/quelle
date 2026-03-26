use chrono::{DateTime, Utc};
#[cfg(feature = "schemars")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, ops::Deref, str::FromStr};

// ── Newtype ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(DateTime<Utc>);

// ── Constructor / accessors ───────────────────────────────────────────────────

impl Timestamp {
    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }

    pub fn to_datetime(self) -> DateTime<Utc> {
        self.0
    }

    pub fn as_datetime(&self) -> &DateTime<Utc> {
        &self.0
    }

    pub fn parse(s: &str) -> Result<Self, chrono::ParseError> {
        s.parse()
    }

    pub fn from_timestamp_millis(millis: i64) -> Option<Self> {
        DateTime::from_timestamp_millis(millis).map(Self)
    }

    pub fn timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        ts.0
    }
}

impl AsRef<DateTime<Utc>> for Timestamp {
    fn as_ref(&self) -> &DateTime<Utc> {
        &self.0
    }
}

impl Deref for Timestamp {
    type Target = DateTime<Utc>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ── Serde: round-trip as RFC 3339 string ─────────────────────────────────────

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| Self(dt.with_timezone(&Utc)))
            .map_err(serde::de::Error::custom)
    }
}

// ── Display / FromStr ─────────────────────────────────────────────────────────

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl FromStr for Timestamp {
    type Err = chrono::ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DateTime::parse_from_rfc3339(s).map(|dt| Self(dt.with_timezone(&Utc)))
    }
}

// ── JsonSchema (schemars 1.x) ─────────────────────────────────────────────────

#[cfg(feature = "schemars")]
impl JsonSchema for Timestamp {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Timestamp".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::Timestamp").into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "format": "date-time",
            "description": "An ISO 8601 / RFC 3339 UTC timestamp, e.g. \"2024-01-15T12:00:00Z\""
        })
    }
}

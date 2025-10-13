use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Strongly typed request identifier backed by ULID.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct RequestId(pub ulid::Ulid);

impl RequestId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }

    pub fn from_ulid(id: ulid::Ulid) -> Self {
        Self(id)
    }

    /// Attempt to parse from a header string; if invalid, generate a new one.
    pub fn from_header_or_new(header_value: Option<&str>) -> Self {
        header_value
            .and_then(|s| s.parse::<RequestId>().ok())
            .unwrap_or_default()
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for RequestId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl FromStr for RequestId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = ulid::Ulid::from_string(s)?;
        Ok(RequestId(id))
    }
}

impl Serialize for RequestId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RequestId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<RequestId>()
            .map_err(|_| serde::de::Error::custom("invalid request id"))
    }
}



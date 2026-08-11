//! Strict SemVer 2.0 value used by the resource release transaction.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<Identifier>,
    build: Option<String>,
}

impl PartialEq for SemanticVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.prerelease == other.prerelease
    }
}

impl Eq for SemanticVersion {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Identifier {
    Numeric(u64),
    Alpha(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid semantic version: {0}")]
pub struct SemanticVersionError(pub String);

impl SemanticVersion {
    pub fn next_patch(&self) -> Self {
        if !self.prerelease.is_empty() {
            return Self {
                major: self.major,
                minor: self.minor,
                patch: self.patch,
                prerelease: vec![],
                build: None,
            };
        }
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch.saturating_add(1),
            prerelease: vec![],
            build: None,
        }
    }

    pub fn initial() -> Self {
        Self {
            major: 0,
            minor: 1,
            patch: 0,
            prerelease: vec![],
            build: None,
        }
    }
}

impl FromStr for SemanticVersion {
    type Err = SemanticVersionError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.is_empty() || raw.trim() != raw {
            return Err(SemanticVersionError(raw.to_string()));
        }
        let (precedence, build) = match raw.split_once('+') {
            Some((left, right)) if !right.is_empty() && !right.contains('+') => {
                validate_identifiers(right, false)?;
                (left, Some(right.to_string()))
            }
            Some(_) => return Err(SemanticVersionError(raw.to_string())),
            None => (raw, None),
        };
        let (core, prerelease) = match precedence.split_once('-') {
            Some((left, right)) if !right.is_empty() => {
                let values = parse_prerelease(right)?;
                (left, values)
            }
            Some(_) => return Err(SemanticVersionError(raw.to_string())),
            None => (precedence, vec![]),
        };
        let mut parts = core.split('.');
        let major = parse_core(parts.next(), raw)?;
        let minor = parse_core(parts.next(), raw)?;
        let patch = parse_core(parts.next(), raw)?;
        if parts.next().is_some() {
            return Err(SemanticVersionError(raw.to_string()));
        }
        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
            build,
        })
    }
}

fn parse_core(value: Option<&str>, raw: &str) -> Result<u64, SemanticVersionError> {
    let value = value.ok_or_else(|| SemanticVersionError(raw.to_string()))?;
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(SemanticVersionError(raw.to_string()));
    }
    value
        .parse()
        .map_err(|_| SemanticVersionError(raw.to_string()))
}

fn parse_prerelease(raw: &str) -> Result<Vec<Identifier>, SemanticVersionError> {
    validate_identifiers(raw, true)?;
    raw.split('.')
        .map(|value| {
            if value.bytes().all(|byte| byte.is_ascii_digit()) {
                if value.len() > 1 && value.starts_with('0') {
                    return Err(SemanticVersionError(raw.to_string()));
                }
                Ok(Identifier::Numeric(
                    value
                        .parse()
                        .map_err(|_| SemanticVersionError(raw.to_string()))?,
                ))
            } else {
                Ok(Identifier::Alpha(value.to_string()))
            }
        })
        .collect()
}

fn validate_identifiers(raw: &str, _prerelease: bool) -> Result<(), SemanticVersionError> {
    if raw.split('.').any(|part| {
        part.is_empty()
            || !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(SemanticVersionError(raw.to_string()));
    }
    Ok(())
}

impl Ord for SemanticVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(
                || match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (false, false) => compare_prerelease(&self.prerelease, &other.prerelease),
                },
            )
    }
}

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_prerelease(left: &[Identifier], right: &[Identifier]) -> Ordering {
    for (a, b) in left.iter().zip(right) {
        let ordering = match (a, b) {
            (Identifier::Numeric(a), Identifier::Numeric(b)) => a.cmp(b),
            (Identifier::Numeric(_), Identifier::Alpha(_)) => Ordering::Less,
            (Identifier::Alpha(_), Identifier::Numeric(_)) => Ordering::Greater,
            (Identifier::Alpha(a), Identifier::Alpha(b)) => a.cmp(b),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.prerelease.is_empty() {
            let values = self
                .prerelease
                .iter()
                .map(|item| match item {
                    Identifier::Numeric(value) => value.to_string(),
                    Identifier::Alpha(value) => value.clone(),
                })
                .collect::<Vec<_>>()
                .join(".");
            write!(formatter, "-{values}")?;
        }
        if let Some(build) = &self.build {
            write!(formatter, "+{build}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SemanticVersion;
    use std::str::FromStr;

    #[test]
    fn accepts_and_orders_semver_precedence_examples() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        let parsed = ordered
            .iter()
            .map(|value| SemanticVersion::from_str(value).unwrap())
            .collect::<Vec<_>>();
        assert!(parsed.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            SemanticVersion::from_str("1.0.0+first").unwrap(),
            SemanticVersion::from_str("1.0.0+second").unwrap()
        );
    }

    #[test]
    fn rejects_non_semver_values() {
        for value in [
            "",
            "1",
            "1.0",
            "01.0.0",
            "1.01.0",
            "1.0.01",
            "v1.0.0",
            "1.0.0-01",
            "1.0.0+",
            "1.0.0 alpha",
            "1.0.0-",
        ] {
            assert!(
                SemanticVersion::from_str(value).is_err(),
                "accepted {value}"
            );
        }
    }

    #[test]
    fn next_patch_promotes_prerelease_before_incrementing() {
        assert_eq!(
            SemanticVersion::from_str("2.1.3-rc.2")
                .unwrap()
                .next_patch()
                .to_string(),
            "2.1.3"
        );
        assert_eq!(
            SemanticVersion::from_str("2.1.3+build")
                .unwrap()
                .next_patch()
                .to_string(),
            "2.1.4"
        );
    }
}

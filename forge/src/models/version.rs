use serde::{Deserialize, Serialize};
use semver::VersionReq as SemverVersionReq;

/// Re-export semver's VersionReq for convenience.
pub type VersionReq = SemverVersionReq;

/// A version range specification for dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRange {
    /// The version requirement string (e.g., "^1.0", ">=0.5, <2.0")
    pub req: String,
}

impl VersionRange {
    pub fn new(req: impl Into<String>) -> Self {
        VersionRange { req: req.into() }
    }

    /// Parse the version range into a semver VersionReq.
    pub fn parse(&self) -> Result<SemverVersionReq, semver::Error> {
        SemverVersionReq::parse(&self.req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_range_parse() {
        let range = VersionRange::new("^1.0");
        let req = range.parse().unwrap();
        assert!(req.matches(&semver::Version::new(1, 5, 0)));
        assert!(!req.matches(&semver::Version::new(2, 0, 0)));
    }
}

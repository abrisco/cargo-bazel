use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::de::Visitor;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Label {
    pub repository: Option<String>,
    pub package: Option<String>,
    pub target: String,
}

impl FromStr for Label {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"^(@[\w\d\-_\.]*)?/{0,2}([\w\d\-_\./]+)?:?([\w\d\-_\.]+)$")?;
        let cap = re.captures(s).context("Failed to parse string as label")?;

        let repository = cap
            .get(1)
            .map(|m| m.as_str().trim_start_matches('@').to_owned());
        let package = cap.get(2).map(|m| m.as_str().to_owned());
        let mut target = cap.get(3).map(|m| m.as_str().to_owned());

        if target.is_none() {
            if let Some(pkg) = &package {
                target = Some(pkg.clone());
            } else if let Some(repo) = &repository {
                target = Some(repo.clone())
            } else {
                bail!("The label is missing a label")
            }
        }

        // The target should be set at this point
        let target = target.unwrap();

        Ok(Self {
            repository,
            package,
            target,
        })
    }
}

impl ToString for Label {
    fn to_string(&self) -> String {
        let mut label = String::new();

        // Add the repository
        if let Some(repo) = &self.repository {
            label = format!("@{}", repo);
        }

        // Add the package
        if let Some(pkg) = &self.package {
            label = format!("{}//{}", label, pkg);
        }

        format!("{}:{}", &label, &self.target)
    }
}

impl Serialize for Label {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.repr())
    }
}

struct LabelVisitor;
impl<'de> Visitor<'de> for LabelVisitor {
    type Value = Label;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expected string value of `{name} {version}`.")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Label::from_str(v).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(LabelVisitor)
    }
}

impl Label {
    pub fn repr(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn full_label() {
        let label = Label::from_str("@repo//package/sub_package:target").unwrap();
        assert_eq!(label.repository.unwrap(), "repo");
        assert_eq!(label.package.unwrap(), "package/sub_package");
        assert_eq!(label.target, "target");
    }

    #[test]
    fn no_repository() {
        let label = Label::from_str("//package:target").unwrap();
        assert_eq!(label.repository, None);
        assert_eq!(label.package.unwrap(), "package");
        assert_eq!(label.target, "target");
    }

    #[test]
    fn no_slashes() {
        let label = Label::from_str("package:target").unwrap();
        assert_eq!(label.repository, None);
        assert_eq!(label.package.unwrap(), "package");
        assert_eq!(label.target, "target");
    }

    #[test]
    fn root_label() {
        let label = Label::from_str("@repo//:target").unwrap();
        assert_eq!(label.repository.unwrap(), "repo");
        assert_eq!(label.package, None);
        assert_eq!(label.target, "target");
    }

    #[test]
    fn root_label_no_repository() {
        let label = Label::from_str("//:target").unwrap();
        assert_eq!(label.repository, None);
        assert_eq!(label.package, None);
        assert_eq!(label.target, "target");
    }

    #[test]
    fn root_label_no_slashes() {
        let label = Label::from_str(":target").unwrap();
        assert_eq!(label.repository, None);
        assert_eq!(label.package, None);
        assert_eq!(label.target, "target");
    }

    #[test]
    fn invalid_double_colon() {
        assert!(Label::from_str("::target").is_err());
    }

    #[test]
    fn invalid_double_at() {
        assert!(Label::from_str("@@repo//pkg:target").is_err());
    }

    #[test]
    #[ignore = "This currently fails. The Label parsing logic needs to be updated"]
    fn invalid_no_double_slash() {
        assert!(Label::from_str("@repo:target").is_err());
    }
}

/// CLI commands for the Forge package registry.
///
/// Usage:
///   forge publish              Publish the current module
///   forge publish --also-crates-io  Publish to both Forge and crates.io
///   forge search <query>       Search for modules
///   forge install <name>       Add a dependency to Forge.toml
///   forge update               Update dependencies
///   forge info <name>          Show module metadata

/// Parsed CLI command.
#[derive(Debug)]
pub enum Command {
    Publish { also_crates_io: bool },
    Search { query: String },
    Install { name: String, version: Option<String> },
    Update,
    Info { name: String },
}

impl Command {
    /// Parse a command from CLI arguments.
    ///
    /// This is a minimal parser; a real implementation would use `clap`.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("");
        match sub {
            "publish" => {
                let also = args.iter().any(|a| a == "--also-crates-io");
                Ok(Command::Publish {
                    also_crates_io: also,
                })
            }
            "search" => {
                let query = args.get(1).ok_or("search requires a query")?;
                Ok(Command::Search {
                    query: query.clone(),
                })
            }
            "install" => {
                let name = args.get(1).ok_or("install requires a package name")?;
                let version = args.get(2).cloned();
                Ok(Command::Install {
                    name: name.clone(),
                    version,
                })
            }
            "update" => Ok(Command::Update),
            "info" => {
                let name = args.get(1).ok_or("info requires a package name")?;
                Ok(Command::Info { name: name.clone() })
            }
            other => Err(format!("unknown command: '{}'", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_publish() {
        let cmd = Command::parse(&args(&["publish"])).unwrap();
        assert!(matches!(cmd, Command::Publish { also_crates_io: false }));
    }

    #[test]
    fn parse_publish_dual() {
        let cmd = Command::parse(&args(&["publish", "--also-crates-io"])).unwrap();
        assert!(matches!(cmd, Command::Publish { also_crates_io: true }));
    }

    #[test]
    fn parse_search() {
        let cmd = Command::parse(&args(&["search", "http"])).unwrap();
        assert!(matches!(cmd, Command::Search { .. }));
    }

    #[test]
    fn parse_install_with_version() {
        let cmd = Command::parse(&args(&["install", "serde", "1.0"])).unwrap();
        match cmd {
            Command::Install { name, version } => {
                assert_eq!(name, "serde");
                assert_eq!(version.as_deref(), Some("1.0"));
            }
            _ => panic!("expected Install"),
        }
    }

    #[test]
    fn unknown_command_errors() {
        assert!(Command::parse(&args(&["frobnicate"])).is_err());
    }
}

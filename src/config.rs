use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

use serde::{Deserialize, Serialize};
use syscalls::Sysno;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ConfigEntry {
    pub allow: Option<BTreeSet<Sysno>>,
    pub block: Option<BTreeSet<Sysno>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Config {
    pub shared_objects: BTreeMap<String, ConfigEntry>,
}

#[derive(Debug)]
pub enum Check {
    Allowed,
    Blocked,
    Unknown,
}

impl Config {
    pub fn check(&self, loc: &str, syscall: Sysno) -> Check {
        match self.shared_objects.get(loc) {
            Some(entry) => {
                if entry
                    .allow
                    .as_ref()
                    .is_some_and(|allowed| allowed.contains(&syscall))
                {
                    Check::Allowed
                } else if entry
                    .block
                    .as_ref()
                    .is_some_and(|blocked| blocked.contains(&syscall))
                {
                    Check::Blocked
                } else {
                    Check::Unknown
                }
            }
            None => Check::Unknown,
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Config {
        let mut file = File::open(path).expect("failed to open file");
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("failed to read file");
        serde_yaml::from_str(&contents).expect("failed to parse config file")
    }

    pub fn new() -> Config {
        Config {
            shared_objects: BTreeMap::new(),
        }
    }
}

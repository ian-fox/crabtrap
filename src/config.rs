use std::collections::{BTreeMap, BTreeSet};

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

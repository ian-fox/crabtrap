use nix::unistd::Pid;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fs, num::ParseIntError, str::FromStr};
use thiserror::Error;

/// Region: one memory region in the process
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Region {
    pub start: u64,
    pub end: u64,
    path: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryMapError {
    #[error("Memory region didn't match regex: {0}")]
    RegexError(String),
    #[error("Failed to parse start of region as u64 from {0}: {1}")]
    ParseIntError(String, ParseIntError),
}

impl FromStr for Region {
    type Err = MemoryMapError;

    fn from_str(s: &str) -> Result<Region, MemoryMapError> {
        let re =
            Regex::new(r"^(?<start>[[:xdigit:]]{12})-(?<end>[[:xdigit:]]{12})[^/\[]*(?<path>.*)$")
                .unwrap();

        let caps = match re.captures(s) {
            Some(caps) => caps,
            None => return Err(MemoryMapError::RegexError(String::from(s))),
        };

        Ok(Region {
            start: match u64::from_str_radix(&caps["start"], 16) {
                Ok(start) => start,
                Err(err) => return Err(MemoryMapError::ParseIntError(String::from(s), err)),
            },
            end: match u64::from_str_radix(&caps["end"], 16) {
                Ok(start) => start,
                Err(err) => return Err(MemoryMapError::ParseIntError(String::from(s), err)),
            },
            path: String::from(&caps["path"]),
        })
    }
}

impl std::fmt::Debug for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Region")
            .field("start", &format_args!("{0:x}", &self.start))
            .field("end", &format_args!("{0:x}", &self.end))
            .field("path", &self.path)
            .finish()
    }
}

/// MemoryMap: selected fields from /proc/{pid}/maps
/// See https://www.man7.org/linux/man-pages/man5/proc.5.html
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MemoryMap {
    pub files: Vec<Region>,
}

impl FromStr for MemoryMap {
    type Err = MemoryMapError;

    fn from_str(s: &str) -> Result<MemoryMap, MemoryMapError> {
        let mut files: Vec<Region> = match s
            .lines()
            .map(Region::from_str)
            .collect::<Result<Vec<Region>, MemoryMapError>>()
        {
            Ok(files) => files
                .into_iter()
                .filter(|region| region.path.starts_with('/'))
                .collect(),
            Err(err) => return Err(err),
        };

        files.sort_by(|a, b| a.start.cmp(&b.start));

        Ok(MemoryMap { files })
    }
}

impl MemoryMap {
    pub fn from_pid(pid: Pid) -> Result<MemoryMap, MemoryMapError> {
        let contents =
            fs::read_to_string(format!("/proc/{pid}/maps")).expect("failed to read maps");

        MemoryMap::from_str(&contents)
    }

    pub fn lookup(&self, addr: u64) -> Option<&str> {
        // If we cared about perf, we could take advantage of this being sorted to exit early
        // But let's not worry about maintaining that invariant for now.
        // It's mostly helpful for the equality check in the unit tests.
        self.files
            .iter()
            .find(|file| file.start <= addr && addr <= file.end)
            .map(|file| file.path.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region() {
        assert_eq!(Region::from_str(&"ffff9f390000-ffff9f517000 r-xp 00000000 fe:01 319964                     /usr/lib/aarch64-linux-gnu/libc.so.6"), Ok(Region {
            start: 0xffff9f390000,
            end: 0xffff9f517000,
            path: String::from("/usr/lib/aarch64-linux-gnu/libc.so.6"),
        }));
    }

    #[test]
    fn test_map() {
        let expected_map = MemoryMap {
            files: vec![
                Region {
                    start: 0xaaaae8e20000,
                    end: 0xaaaae8e29000,
                    path: String::from("/usr/bin/cat"),
                },
                Region {
                    start: 0xaaaae8e3f000,
                    end: 0xaaaae8e40000,
                    path: String::from("/usr/bin/cat"),
                },
                Region {
                    start: 0xaaaae8e40000,
                    end: 0xaaaae8e41000,
                    path: String::from("/usr/bin/cat"),
                },
                Region {
                    start: 0xffff9f390000,
                    end: 0xffff9f517000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/libc.so.6"),
                },
                Region {
                    start: 0xffff9f517000,
                    end: 0xffff9f52c000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/libc.so.6"),
                },
                Region {
                    start: 0xffff9f52c000,
                    end: 0xffff9f530000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/libc.so.6"),
                },
                Region {
                    start: 0xffff9f530000,
                    end: 0xffff9f532000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/libc.so.6"),
                },
                Region {
                    start: 0xffff9f544000,
                    end: 0xffff9f56a000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1"),
                },
                Region {
                    start: 0xffff9f582000,
                    end: 0xffff9f584000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1"),
                },
                Region {
                    start: 0xffff9f584000,
                    end: 0xffff9f586000,
                    path: String::from("/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1"),
                },
            ],
        };

        // Not sure if these are guaranteed to be ordered by start, so I've purposefully moved one around
        assert_eq!(MemoryMap::from_str(&"aaaae8e20000-aaaae8e29000 r-xp 00000000 fe:01 188725                     /usr/bin/cat
aaaae8e3f000-aaaae8e40000 r--p 0000f000 fe:01 188725                     /usr/bin/cat
aaaae8e40000-aaaae8e41000 rw-p 00010000 fe:01 188725                     /usr/bin/cat
aaaaf9cc3000-aaaaf9ce4000 rw-p 00000000 00:00 0                          [heap]
ffff9f36e000-ffff9f390000 rw-p 00000000 00:00 0
ffff9f390000-ffff9f517000 r-xp 00000000 fe:01 319964                     /usr/lib/aarch64-linux-gnu/libc.so.6
ffff9f517000-ffff9f52c000 ---p 00187000 fe:01 319964                     /usr/lib/aarch64-linux-gnu/libc.so.6
ffff9f52c000-ffff9f530000 r--p 0018c000 fe:01 319964                     /usr/lib/aarch64-linux-gnu/libc.so.6
ffff9f532000-ffff9f53f000 rw-p 00000000 00:00 0
ffff9f544000-ffff9f56a000 r-xp 00000000 fe:01 319946                     /usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1
ffff9f530000-ffff9f532000 rw-p 00190000 fe:01 319964                     /usr/lib/aarch64-linux-gnu/libc.so.6
ffff9f575000-ffff9f577000 rw-p 00000000 00:00 0
ffff9f57d000-ffff9f57f000 rw-p 00000000 00:00 0
ffff9f57f000-ffff9f581000 r--p 00000000 00:00 0                          [vvar]
ffff9f581000-ffff9f582000 r-xp 00000000 00:00 0                          [vdso]
ffff9f582000-ffff9f584000 r--p 0002e000 fe:01 319946                     /usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1
ffff9f584000-ffff9f586000 rw-p 00030000 fe:01 319946                     /usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1
fffff69fe000-fffff6a1f000 rw-p 00000000 00:00 0                          [stack]"), Ok(expected_map.clone()));

        assert_eq!(
            expected_map.lookup(0xffff9f582004),
            Some("/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1"),
        );
        assert_eq!(expected_map.lookup(0x1234), None);
    }
}

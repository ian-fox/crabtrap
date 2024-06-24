use crabtrap::{ChildExit, Config, ConfigEntry};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use syscalls::Sysno;

#[test]
fn test_ok() {
    for bin in ["static", "dynamic", "all-in-one"] {
        assert_eq!(
            crabtrap::execute(
                &CString::new(format!("/usr/local/bin/{}", bin)).unwrap(),
                &[],
                &[&CString::new("LD_LIBRARY_PATH=/usr/local/lib").unwrap()],
                &Config {
                    shared_objects: BTreeMap::new(),
                },
            ),
            ChildExit::Exited(0),
        );
    }
}

#[test]
fn test_blocked() {
    for bin in ["static", "dynamic"] {
        assert_eq!(
            crabtrap::execute(
                &CString::new(format!("/usr/local/bin/{}", bin)).unwrap(),
                &[],
                &[&CString::new("LD_LIBRARY_PATH=/usr/local/lib").unwrap()],
                &Config {
                    shared_objects: BTreeMap::from([(
                        "/usr/local/lib/libprintf_wrapper.so".into(),
                        ConfigEntry {
                            allow: None,
                            block: Some(BTreeSet::from([Sysno::write])),
                        }
                    )]),
                },
            ),
            ChildExit::IllegalSyscall(Sysno::write, "/usr/local/lib/libprintf_wrapper.so".into()),
        );
    }
}

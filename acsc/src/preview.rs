//! Human-readable JSON for ordinary paths/argv, with lossless native encoding
//! retained for the occasional non-Unicode OS string.

use std::ffi::{OsStr, OsString};

use serde::{Serialize, Serializer};

use crate::domain::Selection;

struct ReadableOs<'a>(&'a OsStr);

impl Serialize for ReadableOs<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(text) = self.0.to_str() {
            serializer.serialize_str(text)
        } else {
            self.0.serialize(serializer)
        }
    }
}

pub fn os_string<S: Serializer>(value: &OsStr, serializer: S) -> Result<S::Ok, S::Error> {
    ReadableOs(value).serialize(serializer)
}

pub fn args<S: Serializer>(value: &[OsString], serializer: S) -> Result<S::Ok, S::Error> {
    value
        .iter()
        .map(|value| ReadableOs(value))
        .collect::<Vec<_>>()
        .serialize(serializer)
}

pub fn command<S: Serializer>(
    value: &Selection<Vec<OsString>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    Selection::new(
        value
            .value
            .iter()
            .map(|value| ReadableOs(value))
            .collect::<Vec<_>>(),
        value.source,
    )
    .serialize(serializer)
}

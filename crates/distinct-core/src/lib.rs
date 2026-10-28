//! Distinct core: records, hashes, schemas, and state.
//!
//! WO-01 adds closed schemas, domain-separated hashing, duplicate-key
//! rejection, ID validation, and the record envelope types.

pub mod dup_keys;
pub mod export;
pub mod extract;
pub mod governance;
pub mod hash;
pub mod id;
pub mod packet;
pub mod patch;
pub mod record;
pub mod review_packet;
pub mod schema;
pub mod store;
pub mod test_bench;
pub mod tokens;

/// Native protocol identifier probed by the Node launcher (contract section 5).
pub const PROTOCOL_VERSION: &str = "distinct/1.0";

/// Engine version, equal to the workspace package version.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Process exit codes (contract section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Completed utility.
    Ok = 0,
    /// Confirmed configured check failure or changes requested.
    CheckFailed = 1,
    /// Incomplete, or a required capability is unavailable.
    Unavailable = 2,
    /// Invalid input, conflict, or execution failure.
    Invalid = 3,
}

impl Exit {
    /// The numeric process exit status.
    #[must_use]
    pub fn code(self) -> i32 {
        self as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_contract() {
        assert_eq!(Exit::Ok.code(), 0);
        assert_eq!(Exit::CheckFailed.code(), 1);
        assert_eq!(Exit::Unavailable.code(), 2);
        assert_eq!(Exit::Invalid.code(), 3);
    }

    #[test]
    fn protocol_id_is_frozen() {
        assert_eq!(PROTOCOL_VERSION, "distinct/1.0");
    }
}

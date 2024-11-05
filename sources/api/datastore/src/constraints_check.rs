//! The outcome of the constraint check determines whether the transaction can proceed to commit.
//! A ‘rejected’ result means that one or more constraints have not been satisfied,
//! preventing the transaction from being committed. On the other hand, an ‘approved’
//! result confirms that all constraints are satisfied and provides the required
//! settings and metadata for the commit.
//! Constraint checks can alter the write.

use std::collections::HashMap;

use crate::{error, Key};

type RejectReason = String;

/// Represents a successful write operation after constraints have been approved.
/// Contains the following fields:
/// - `settings`: A collection of key-value pairs representing the settings to be committed.
/// - `metadata`: A collection of metadata entries.
#[derive(PartialEq)]
pub struct ApprovedWrite {
    pub settings: HashMap<Key, String>,
    pub metadata: Vec<(Key, Key, String)>,
}

/// Represents the result of a constraint check.
/// The result can either reject the operation or approve it with the required data.
#[derive(PartialEq)]
pub enum ConstraintCheckResult {
    Reject(RejectReason),
    Approve(ApprovedWrite),
}

impl TryFrom<ConstraintCheckResult> for ApprovedWrite {
    type Error = error::Error;

    fn try_from(constraint_check_result: ConstraintCheckResult) -> Result<Self, Self::Error> {
        match constraint_check_result {
            ConstraintCheckResult::Reject(err) => error::ConstraintCheckRejectSnafu { err }.fail(),
            ConstraintCheckResult::Approve(approved_write) => Ok(approved_write),
        }
    }
}

impl From<Option<ApprovedWrite>> for ConstraintCheckResult {
    fn from(approved_write: Option<ApprovedWrite>) -> Self {
        match approved_write {
            None => ConstraintCheckResult::Reject(
                "The write for the given transaction is rejected".to_string(),
            ),
            Some(approved_write) => ConstraintCheckResult::Approve(approved_write),
        }
    }
}

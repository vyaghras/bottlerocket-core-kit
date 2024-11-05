use std::collections::HashMap;

use crate::Key;

pub struct ApprovedWrite {
    pub settings: HashMap<Key, String>,
    pub metadata: Vec<(Key, Key, String)>,
}

pub enum ConstraintCheckResult {
    Reject,
    Approve(ApprovedWrite),
}

impl From<ConstraintCheckResult> for Option<ApprovedWrite> {
    fn from(constraint_check_result: ConstraintCheckResult) -> Self {
        match constraint_check_result {
            ConstraintCheckResult::Reject => None,
            ConstraintCheckResult::Approve(approved_write) => Some(approved_write),
        }
    }
}
impl From<Option<ApprovedWrite>> for ConstraintCheckResult {
    fn from(approved_write: Option<ApprovedWrite>) -> Self {
        match approved_write {
            None => ConstraintCheckResult::Reject,
            Some(approved_write) => ConstraintCheckResult::Approve(approved_write),
        }
    }
}

//! Per-client job templates, difficulty notifications, and `mining.notify` dispatch.
//!
//! Split into [`difficulty`] (`mining.set_difficulty`), [`immediate_job`] (first job after subscribe),
//! and [`new_block_job`] (template refresh / vardiff).

mod difficulty;
mod immediate_job;
mod new_block_job;

pub(crate) use difficulty::send_client_diff;
pub(crate) use immediate_job::send_immediate_job_task;
pub(crate) use new_block_job::new_block_job_task;

use once_cell::sync::Lazy;
use regex::Regex;
use std::time::Duration;

pub(crate) static BIG_JOB_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r".*(BzMiner|IceRiverMiner).*").unwrap());

pub(crate) const BALANCE_DELAY: Duration = Duration::from_secs(60);
pub(crate) const CLIENT_TIMEOUT: Duration = Duration::from_secs(20);

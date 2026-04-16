use prometheus::proto::{LabelPair, MetricFamily};

use super::types::WorkerInfo;

pub(super) fn parse_worker_labels(labels: &[LabelPair]) -> (String, String, String) {
    let mut instance = String::new();
    let mut worker = String::new();
    let mut wallet = String::new();

    for label in labels {
        match label.get_name() {
            "instance" => instance = label.get_value().to_string(),
            "worker" => worker = label.get_value().to_string(),
            "wallet" => wallet = label.get_value().to_string(),
            _ => {}
        }
    }

    (instance, worker, wallet)
}

pub(super) fn parse_instance_wallet_labels(labels: &[LabelPair]) -> (String, String) {
    let mut instance = String::new();
    let mut wallet = String::new();
    for label in labels {
        match label.get_name() {
            "instance" => instance = label.get_value().to_string(),
            "wallet" => wallet = label.get_value().to_string(),
            _ => {}
        }
    }
    (instance, wallet)
}

pub(super) fn sum_prometheus_counter_family(families: &[MetricFamily], name: &str) -> u64 {
    let mut sum = 0u64;
    for family in families {
        if family.get_name() != name {
            continue;
        }
        for m in family.get_metric() {
            sum = sum.saturating_add(m.get_counter().get_value().max(0.0) as u64);
        }
    }
    sum
}

pub(super) fn new_worker_info(instance: String, worker: String, wallet: String) -> WorkerInfo {
    WorkerInfo {
        instance,
        worker,
        wallet,
        hashrate: 0.0,
        shares: 0,
        stale: 0,
        invalid: 0,
        duplicate_shares: 0,
        weak_shares: 0,
        blocks: 0,
        disconnects: 0,
        jobs: 0,
        balance_kas: None,
        errors: 0,
        blocks_accepted_by_node: 0,
        blocks_not_confirmed_blue: 0,
        last_seen: None,
        status: None,
        current_difficulty: None,
        session_uptime: None,
    }
}

use metrics::{counter, gauge, histogram};

pub fn record_block_committed(tx_count: u64, log_count: u64) {
    counter!("chainlens_blocks_indexed_total").increment(1);
    counter!("chainlens_transactions_indexed_total").increment(tx_count);
    counter!("chainlens_logs_indexed_total").increment(log_count);
}

pub fn record_rpc_request(method: &str, duration_secs: f64, success: bool) {
    histogram!("chainlens_rpc_request_duration_seconds", "method" => method.to_string())
        .record(duration_secs);
    counter!("chainlens_rpc_requests_total", "method" => method.to_string(), "outcome" => if success { "success" } else { "error" })
        .increment(1);
}

pub fn record_db_commit(duration_secs: f64) {
    histogram!("chainlens_db_commit_duration_seconds").record(duration_secs);
}

pub fn set_indexing_lag(lag: u64) {
    gauge!("chainlens_indexing_lag_blocks").set(lag as f64);
}

pub fn record_reorg(depth: u64) {
    counter!("chainlens_reorgs_total").increment(1);
    histogram!("chainlens_reorg_depth").record(depth as f64);
}

pub fn set_worker_count(busy: u64, total: u64) {
    gauge!("chainlens_workers_busy").set(busy as f64);
    gauge!("chainlens_workers_total").set(total as f64);
}

pub fn record_transaction_decoded() {
    counter!("chainlens_transactions_decoded_total").increment(1);
}

pub fn record_transaction_actions_generated(count: u64) {
    counter!("chainlens_transaction_actions_generated_total").increment(count);
}

pub fn record_address_analyzed() {
    counter!("chainlens_addresses_analyzed_total").increment(1);
}

pub fn record_graph_generated(node_count: u64, edge_count: u64) {
    counter!("chainlens_graphs_generated_total").increment(1);
    counter!("chainlens_graph_nodes_total").increment(node_count);
    counter!("chainlens_graph_edges_total").increment(edge_count);
}

pub fn record_anomaly_detected(severity: &str) {
    counter!("chainlens_anomalies_detected_total", "severity" => severity.to_string()).increment(1);
}

pub fn record_mev_candidate_detected(mev_type: &str) {
    counter!("chainlens_mev_candidates_detected_total", "type" => mev_type.to_string())
        .increment(1);
}

pub fn record_intelligence_processing_duration(duration_secs: f64, operation: &str) {
    histogram!("chainlens_intelligence_processing_duration_seconds", "operation" => operation.to_string())
        .record(duration_secs);
}

pub fn record_contract_analyzed() {
    counter!("chainlens_contracts_analyzed_total").increment(1);
}

pub fn record_block_analytics_generated() {
    counter!("chainlens_block_analytics_generated_total").increment(1);
}

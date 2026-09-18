use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationshipType {
    TransferredTo,
    ReceivedFrom,
    Called,
    Approved,
    Deployed,
    InteractedWith,
    TransferredTokenTo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub address: String,
    pub label: Option<String>,
    pub interaction_count: i64,
    pub first_interaction: Option<String>,
    pub last_interaction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relationship: RelationshipType,
    pub count: i64,
    pub total_value: Option<String>,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressGraph {
    pub center: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub depth_reached: u32,
    pub total_nodes: usize,
    pub total_edges: usize,
}

pub async fn build_graph(
    pool: &PgPool,
    address: &str,
    depth: u32,
    limit: u32,
) -> Result<AddressGraph, sqlx::Error> {
    let addr_bytes =
        hex::decode(address.trim_start_matches("0x")).map_err(|_| sqlx::Error::RowNotFound)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    visited.insert(addr_bytes.clone());
    queue.push_back((addr_bytes.clone(), 0u32));

    while let Some((current_addr, current_depth)) = queue.pop_front() {
        if current_depth >= depth || nodes.len() >= limit as usize {
            break;
        }

        let sent_txs = sqlx::query(
            "SELECT to_addr, COUNT(*) as count,
                    COALESCE(SUM(value::numeric), 0)::text as total_value,
                    MIN(b.timestamp::text) as first_seen,
                    MAX(b.timestamp::text) as last_seen
             FROM transactions t
             JOIN blocks b ON b.number = t.block_number
             WHERE t.from_addr = $1 AND t.to_addr IS NOT NULL
             GROUP BY to_addr
             ORDER BY count DESC
             LIMIT 20",
        )
        .bind(&current_addr)
        .fetch_all(pool)
        .await?;

        for row in &sent_txs {
            let to_addr: Vec<u8> = row.get("to_addr");
            let count: i64 = row.get("count");
            let total_value: String = row.get("total_value");
            let first_seen: Option<String> = row.get("first_seen");
            let last_seen: Option<String> = row.get("last_seen");

            let to_hex = format!("0x{}", hex::encode(&to_addr));

            edges.push(GraphEdge {
                from: format!("0x{}", hex::encode(&current_addr)),
                to: to_hex.clone(),
                relationship: RelationshipType::TransferredTo,
                count,
                total_value: Some(wei_to_eth(&total_value)),
                first_seen,
                last_seen,
            });

            if !visited.contains(&to_addr) && nodes.len() < limit as usize {
                visited.insert(to_addr.clone());
                queue.push_back((to_addr.clone(), current_depth + 1));

                let node_info = sqlx::query(
                    "SELECT COUNT(*) as tx_count FROM transactions WHERE from_addr = $1",
                )
                .bind(&to_addr)
                .fetch_one(pool)
                .await?;
                let tx_count: i64 = node_info.get("tx_count");

                nodes.push(GraphNode {
                    address: to_hex,
                    label: None,
                    interaction_count: tx_count,
                    first_interaction: None,
                    last_interaction: None,
                });
            }
        }

        let received_txs = sqlx::query(
            "SELECT from_addr, COUNT(*) as count,
                    COALESCE(SUM(value::numeric), 0)::text as total_value,
                    MIN(b.timestamp::text) as first_seen,
                    MAX(b.timestamp::text) as last_seen
             FROM transactions t
             JOIN blocks b ON b.number = t.block_number
             WHERE t.to_addr = $1
             GROUP BY from_addr
             ORDER BY count DESC
             LIMIT 20",
        )
        .bind(&current_addr)
        .fetch_all(pool)
        .await?;

        for row in &received_txs {
            let from_addr: Vec<u8> = row.get("from_addr");
            let count: i64 = row.get("count");
            let total_value: String = row.get("total_value");
            let first_seen: Option<String> = row.get("first_seen");
            let last_seen: Option<String> = row.get("last_seen");

            let from_hex = format!("0x{}", hex::encode(&from_addr));

            edges.push(GraphEdge {
                from: from_hex.clone(),
                to: format!("0x{}", hex::encode(&current_addr)),
                relationship: RelationshipType::ReceivedFrom,
                count,
                total_value: Some(wei_to_eth(&total_value)),
                first_seen,
                last_seen,
            });

            if !visited.contains(&from_addr) && nodes.len() < limit as usize {
                visited.insert(from_addr.clone());
                queue.push_back((from_addr.clone(), current_depth + 1));

                nodes.push(GraphNode {
                    address: from_hex,
                    label: None,
                    interaction_count: count,
                    first_interaction: None,
                    last_interaction: None,
                });
            }
        }

        let token_transfers = sqlx::query(
            "SELECT to_addr, COUNT(*) as count
             FROM token_transfers
             WHERE from_addr = $1
             GROUP BY to_addr
             ORDER BY count DESC
             LIMIT 10",
        )
        .bind(&current_addr)
        .fetch_all(pool)
        .await?;

        for row in &token_transfers {
            let to_addr: Vec<u8> = row.get("to_addr");
            let count: i64 = row.get("count");
            let to_hex = format!("0x{}", hex::encode(&to_addr));

            edges.push(GraphEdge {
                from: format!("0x{}", hex::encode(&current_addr)),
                to: to_hex,
                relationship: RelationshipType::TransferredTokenTo,
                count,
                total_value: None,
                first_seen: None,
                last_seen: None,
            });
        }
    }

    let total_nodes = nodes.len();
    let total_edges = edges.len();

    Ok(AddressGraph {
        center: address.to_string(),
        nodes,
        edges,
        depth_reached: depth.min(current_depth(&queue)),
        total_nodes,
        total_edges,
    })
}

fn current_depth(queue: &std::collections::VecDeque<(Vec<u8>, u32)>) -> u32 {
    queue.back().map(|(_, d)| *d).unwrap_or(0)
}

fn wei_to_eth(wei: &str) -> String {
    let wei_val: u128 = wei.parse().unwrap_or(0);
    let eth = wei_val as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relationship_type_serialization() {
        assert_eq!(
            serde_json::to_string(&RelationshipType::TransferredTo).unwrap(),
            "\"TRANSFERRED_TO\""
        );
        assert_eq!(
            serde_json::to_string(&RelationshipType::ReceivedFrom).unwrap(),
            "\"RECEIVED_FROM\""
        );
    }

    #[test]
    fn graph_node_creation() {
        let node = GraphNode {
            address: "0x1234".to_string(),
            label: Some("Test".to_string()),
            interaction_count: 5,
            first_interaction: None,
            last_interaction: None,
        };
        assert_eq!(node.address, "0x1234");
        assert_eq!(node.interaction_count, 5);
    }

    #[test]
    fn graph_edge_creation() {
        let edge = GraphEdge {
            from: "0x1111".to_string(),
            to: "0x2222".to_string(),
            relationship: RelationshipType::TransferredTo,
            count: 10,
            total_value: Some("1.5".to_string()),
            first_seen: Some("2024-01-01".to_string()),
            last_seen: Some("2024-01-15".to_string()),
        };
        assert_eq!(edge.from, "0x1111");
        assert_eq!(edge.count, 10);
    }

    #[test]
    fn all_relationship_types_serialize() {
        let types = vec![
            RelationshipType::TransferredTo,
            RelationshipType::ReceivedFrom,
            RelationshipType::Called,
            RelationshipType::Approved,
            RelationshipType::Deployed,
            RelationshipType::InteractedWith,
            RelationshipType::TransferredTokenTo,
        ];
        for rt in types {
            let json = serde_json::to_string(&rt).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn graph_structure() {
        let graph = AddressGraph {
            center: "0xcenter".to_string(),
            nodes: vec![GraphNode {
                address: "0x1111".to_string(),
                label: None,
                interaction_count: 5,
                first_interaction: None,
                last_interaction: None,
            }],
            edges: vec![GraphEdge {
                from: "0xcenter".to_string(),
                to: "0x1111".to_string(),
                relationship: RelationshipType::TransferredTo,
                count: 3,
                total_value: None,
                first_seen: None,
                last_seen: None,
            }],
            depth_reached: 1,
            total_nodes: 1,
            total_edges: 1,
        };
        assert_eq!(graph.center, "0xcenter");
        assert_eq!(graph.total_nodes, 1);
        assert_eq!(graph.total_edges, 1);
    }
}

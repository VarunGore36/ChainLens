use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiDoc {
    pub name: String,
    pub version: String,
    pub description: String,
    pub endpoints: Vec<EndpointDoc>,
}

#[derive(Debug, Serialize)]
pub struct EndpointDoc {
    pub path: String,
    pub method: String,
    pub description: String,
    pub parameters: Vec<ParamDoc>,
}

#[derive(Debug, Serialize)]
pub struct ParamDoc {
    pub name: String,
    pub location: String,
    pub required: bool,
    pub description: String,
}

pub async fn api_docs() -> Json<ApiDoc> {
    Json(ApiDoc {
        name: "ChainLens Intelligence API".to_string(),
        version: "0.1.0".to_string(),
        description: "Ethereum Intelligence & Forensics Engine".to_string(),
        endpoints: vec![
            EndpointDoc {
                path: "/block/{number_or_hash}".to_string(),
                method: "GET".to_string(),
                description: "Get block by number or hash".to_string(),
                parameters: vec![ParamDoc {
                    name: "number_or_hash".to_string(),
                    location: "path".to_string(),
                    required: true,
                    description: "Block number or hash".to_string(),
                }],
            },
            EndpointDoc {
                path: "/transaction/{hash}".to_string(),
                method: "GET".to_string(),
                description: "Get transaction by hash".to_string(),
                parameters: vec![ParamDoc {
                    name: "hash".to_string(),
                    location: "path".to_string(),
                    required: true,
                    description: "Transaction hash".to_string(),
                }],
            },
            EndpointDoc {
                path: "/api/v1/transactions/{hash}/explain".to_string(),
                method: "GET".to_string(),
                description: "Get decoded transaction actions and summary".to_string(),
                parameters: vec![ParamDoc {
                    name: "hash".to_string(),
                    location: "path".to_string(),
                    required: true,
                    description: "Transaction hash".to_string(),
                }],
            },
            EndpointDoc {
                path: "/api/v1/addresses/{address}/intelligence".to_string(),
                method: "GET".to_string(),
                description: "Get address intelligence and behavior classification".to_string(),
                parameters: vec![ParamDoc {
                    name: "address".to_string(),
                    location: "path".to_string(),
                    required: true,
                    description: "Ethereum address".to_string(),
                }],
            },
            EndpointDoc {
                path: "/api/v1/addresses/{address}/graph".to_string(),
                method: "GET".to_string(),
                description: "Get address relationship graph".to_string(),
                parameters: vec![
                    ParamDoc {
                        name: "address".to_string(),
                        location: "path".to_string(),
                        required: true,
                        description: "Ethereum address".to_string(),
                    },
                    ParamDoc {
                        name: "depth".to_string(),
                        location: "query".to_string(),
                        required: false,
                        description: "Traversal depth (1-5, default 2)".to_string(),
                    },
                    ParamDoc {
                        name: "limit".to_string(),
                        location: "query".to_string(),
                        required: false,
                        description: "Max nodes (1-200, default 50)".to_string(),
                    },
                ],
            },
            EndpointDoc {
                path: "/api/v1/contracts/{address}/intelligence".to_string(),
                method: "GET".to_string(),
                description: "Get contract intelligence".to_string(),
                parameters: vec![ParamDoc {
                    name: "address".to_string(),
                    location: "path".to_string(),
                    required: true,
                    description: "Contract address".to_string(),
                }],
            },
            EndpointDoc {
                path: "/api/v1/anomalies".to_string(),
                method: "GET".to_string(),
                description: "Get detected anomalies".to_string(),
                parameters: vec![],
            },
            EndpointDoc {
                path: "/api/v1/blocks/{number}/analytics".to_string(),
                method: "GET".to_string(),
                description: "Get block analytics and MEV candidates".to_string(),
                parameters: vec![ParamDoc {
                    name: "number".to_string(),
                    location: "path".to_string(),
                    required: true,
                    description: "Block number".to_string(),
                }],
            },
            EndpointDoc {
                path: "/api/v1/addresses/{address}/export".to_string(),
                method: "GET".to_string(),
                description: "Export address transactions as JSON or CSV".to_string(),
                parameters: vec![
                    ParamDoc {
                        name: "address".to_string(),
                        location: "path".to_string(),
                        required: true,
                        description: "Ethereum address".to_string(),
                    },
                    ParamDoc {
                        name: "format".to_string(),
                        location: "query".to_string(),
                        required: false,
                        description: "Export format: json or csv (default json)".to_string(),
                    },
                    ParamDoc {
                        name: "limit".to_string(),
                        location: "query".to_string(),
                        required: false,
                        description: "Max records (1-10000, default 1000)".to_string(),
                    },
                ],
            },
            EndpointDoc {
                path: "/api/v1/addresses/{address}/cluster".to_string(),
                method: "GET".to_string(),
                description: "Find address cluster".to_string(),
                parameters: vec![
                    ParamDoc {
                        name: "address".to_string(),
                        location: "path".to_string(),
                        required: true,
                        description: "Ethereum address".to_string(),
                    },
                    ParamDoc {
                        name: "depth".to_string(),
                        location: "query".to_string(),
                        required: false,
                        description: "Cluster depth (1-5, default 2)".to_string(),
                    },
                ],
            },
            EndpointDoc {
                path: "/ws".to_string(),
                method: "GET".to_string(),
                description: "WebSocket for real-time updates".to_string(),
                parameters: vec![],
            },
            EndpointDoc {
                path: "/health".to_string(),
                method: "GET".to_string(),
                description: "Health check".to_string(),
                parameters: vec![],
            },
            EndpointDoc {
                path: "/status".to_string(),
                method: "GET".to_string(),
                description: "Indexer status (cursor, finalized, lag)".to_string(),
                parameters: vec![],
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn api_docs_returns_valid_json() {
        let docs = api_docs().await;
        assert!(!docs.endpoints.is_empty());
        assert_eq!(docs.name, "ChainLens Intelligence API");
    }
}

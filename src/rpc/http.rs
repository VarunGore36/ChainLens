use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use alloy_primitives::B256;
use reqwest::Client;
use serde::de::DeserializeOwned;

use super::ratelimit::RateLimit;
use super::retry::RetryPolicy;
use super::types::{
    BlockResponse, JsonRpcError, JsonRpcRequest, JsonRpcResponse, LogResponse, ReceiptResponse,
    TransactionResponse,
};
use super::{BlockTag, EthClient, RpcError};

#[derive(Debug, Clone)]
pub struct HttpRpcConfig {
    pub url: String,
    pub rate_limit: RateLimit,
    pub retry: RetryPolicy,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct HttpRpcClient {
    client: Client,
    url: String,
    rate_limit: RateLimit,
    retry: RetryPolicy,
    supports_block_receipts: bool,
    next_id: std::sync::Arc<AtomicU64>,
}

impl HttpRpcClient {
    pub async fn new(config: HttpRpcConfig) -> Result<Self, RpcError> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| RpcError::Connection(format!("failed to build HTTP client: {e}")))?;

        let mut rpc = Self {
            client,
            url: config.url,
            rate_limit: config.rate_limit,
            retry: config.retry,
            supports_block_receipts: false,
            next_id: std::sync::Arc::new(AtomicU64::new(1)),
        };

        rpc.probe_block_receipts().await;

        Ok(rpc)
    }

    async fn probe_block_receipts(&mut self) {
        let block_number = match self.fetch_block_number(BlockTag::Latest).await {
            Ok(n) => n,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "could not probe eth_getBlockReceipts: failed to get latest block"
                );
                return;
            }
        };

        match self.fetch_block_receipts_raw(block_number).await {
            Ok(_) => {
                self.supports_block_receipts = true;
                tracing::info!("eth_getBlockReceipts supported; using batched receipts");
            }
            Err(RpcError::JsonRpc { code: -32601, .. }) => {
                tracing::info!(
                    "eth_getBlockReceipts not supported; \
                     falling back to per-transaction receipts"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "eth_getBlockReceipts probe returned unexpected error; assuming unsupported"
                );
            }
        }
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, RpcError> {
        let mut last_error = None;

        for attempt in 0..=self.retry.max_retries {
            if attempt > 0 {
                let delay = self.retry.delay_for_attempt(attempt - 1);
                tracing::debug!(
                    method,
                    attempt,
                    delay_ms = delay.as_millis(),
                    "retrying RPC call"
                );
                tokio::time::sleep(delay).await;
            }

            self.rate_limit.wait().await;

            match self.execute_once(method, params.clone()).await {
                Ok(value) => return Ok(value),
                Err(e) if e.is_retryable() && attempt < self.retry.max_retries => {
                    last_error = Some(e);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(RpcError::Connection(
            "retries exhausted with no error".to_owned(),
        )))
    }

    async fn execute_once<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_owned(),
            params,
            id,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RpcError::Timeout
                } else {
                    RpcError::Connection(format!("{e}"))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(RpcError::Http {
                status: status.as_u16(),
                body,
            });
        }

        let rpc_response: JsonRpcResponse<T> = response.json().await.map_err(|e| {
            RpcError::Deserialize(format!("failed to parse JSON-RPC response: {e}"))
        })?;

        if let Some(JsonRpcError { code, message }) = rpc_response.error {
            return Err(RpcError::JsonRpc { code, message });
        }

        rpc_response
            .result
            .ok_or_else(|| RpcError::Deserialize("JSON-RPC response missing result".to_owned()))
    }

    async fn fetch_block_number(&self, tag: BlockTag) -> Result<u64, RpcError> {
        let block: Option<BlockResponse> = self
            .call(
                "eth_getBlockByNumber",
                serde_json::json!([tag.as_str(), false]),
            )
            .await?;
        block
            .map(|b| b.number)
            .ok_or_else(|| RpcError::Deserialize(format!("block tag {tag:?} returned null")))
    }

    async fn fetch_block_receipts_raw(
        &self,
        number: u64,
    ) -> Result<Vec<ReceiptResponse>, RpcError> {
        self.call(
            "eth_getBlockReceipts",
            serde_json::json!([format!("0x{number:x}")]),
        )
        .await
    }
}

#[async_trait::async_trait]
impl EthClient for HttpRpcClient {
    async fn get_block_by_number(
        &self,
        number: u64,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError> {
        self.call(
            "eth_getBlockByNumber",
            serde_json::json!([format!("0x{number:x}"), full_transactions]),
        )
        .await
    }

    async fn get_block_by_tag(
        &self,
        tag: BlockTag,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError> {
        self.call(
            "eth_getBlockByNumber",
            serde_json::json!([tag.as_str(), full_transactions]),
        )
        .await
    }

    async fn get_block_number(&self, tag: BlockTag) -> Result<u64, RpcError> {
        let block: Option<BlockResponse> = self
            .call(
                "eth_getBlockByNumber",
                serde_json::json!([tag.as_str(), false]),
            )
            .await?;
        block
            .map(|b| b.number)
            .ok_or_else(|| RpcError::Deserialize(format!("block tag {:?} returned null", tag)))
    }

    async fn get_block_receipts(&self, number: u64) -> Result<Vec<ReceiptResponse>, RpcError> {
        self.fetch_block_receipts_raw(number).await
    }

    async fn get_transaction_receipt(
        &self,
        hash: B256,
    ) -> Result<Option<ReceiptResponse>, RpcError> {
        self.call("eth_getTransactionReceipt", serde_json::json!([hash]))
            .await
    }

    fn supports_block_receipts(&self) -> bool {
        self.supports_block_receipts
    }

    async fn get_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, RpcError> {
        self.call("eth_getTransactionByHash", serde_json::json!([hash]))
            .await
    }

    async fn get_logs_for_transaction(&self, tx_hash: B256) -> Result<Vec<LogResponse>, RpcError> {
        let receipt = self.get_transaction_receipt(tx_hash).await?;
        Ok(receipt.map(|r| r.logs).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::BlockId;

    #[test]
    fn rpc_error_retry_classification() {
        assert!(
            RpcError::Http {
                status: 429,
                body: "rate limited".to_owned(),
            }
            .is_retryable()
        );

        assert!(
            RpcError::Http {
                status: 500,
                body: "internal error".to_owned(),
            }
            .is_retryable()
        );

        assert!(
            RpcError::Http {
                status: 503,
                body: "unavailable".to_owned(),
            }
            .is_retryable()
        );

        assert!(
            !RpcError::Http {
                status: 400,
                body: "bad request".to_owned(),
            }
            .is_retryable()
        );

        assert!(
            !RpcError::Http {
                status: 404,
                body: "not found".to_owned(),
            }
            .is_retryable()
        );

        assert!(RpcError::Timeout.is_retryable());
        assert!(RpcError::Connection("refused".to_owned()).is_retryable());

        assert!(
            !RpcError::JsonRpc {
                code: -32601,
                message: "method not found".to_owned(),
            }
            .is_retryable()
        );

        assert!(!RpcError::Deserialize("bad json".to_owned()).is_retryable());
    }

    #[test]
    fn block_id_tag_serialization() {
        assert_eq!(
            serde_json::to_string(&BlockId::Tag(BlockTag::Latest)).unwrap(),
            "\"latest\""
        );
        assert_eq!(
            serde_json::to_string(&BlockId::Tag(BlockTag::Safe)).unwrap(),
            "\"safe\""
        );
        assert_eq!(
            serde_json::to_string(&BlockId::Tag(BlockTag::Finalized)).unwrap(),
            "\"finalized\""
        );
    }

    #[test]
    fn block_id_number_serialization() {
        assert_eq!(
            serde_json::to_string(&BlockId::Number(0)).unwrap(),
            "\"0x0\""
        );
        assert_eq!(
            serde_json::to_string(&BlockId::Number(26)).unwrap(),
            "\"0x1a\""
        );
        assert_eq!(
            serde_json::to_string(&BlockId::Number(20_000_000)).unwrap(),
            "\"0x1312d00\""
        );
    }

    #[test]
    fn block_tag_as_str() {
        assert_eq!(BlockTag::Latest.as_str(), "latest");
        assert_eq!(BlockTag::Safe.as_str(), "safe");
        assert_eq!(BlockTag::Finalized.as_str(), "finalized");
        assert_eq!(BlockTag::Earliest.as_str(), "earliest");
        assert_eq!(BlockTag::Pending.as_str(), "pending");
    }
}

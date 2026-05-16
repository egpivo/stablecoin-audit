//! JSON-RPC mocks for offline unit tests (wiremock).

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub async fn spawn_usdc_rpc() -> MockServer {
        let server = MockServer::start().await;
        let addr = "0x0000000000000000000000000000000000000001";
        let block_hex = "0x16e3600";

        for (rpc_method, result) in [
            ("eth_chainId", json!("0x1")),
            ("eth_blockNumber", json!(block_hex)),
            (
                "eth_getBlockByNumber",
                json!({
                    "number": block_hex,
                    "timestamp": "0x66584000",
                    "hash": "0xabc",
                }),
            ),
            ("eth_getLogs", json!([])),
        ] {
            Mock::given(method("POST"))
                .and(body_string_contains(rpc_method))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": result
                })))
                .mount(&server)
                .await;
        }

        // eth_call: name(), symbol(), decimals(), totalSupply() at various blocks
        Mock::given(method("POST"))
            .and(body_string_contains("eth_call"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": format!("0x0000000000000000000000000000000000000000000000000000000000000000")
            })))
            .mount(&server)
            .await;

        let _ = addr; // silence unused in case we specialize calls later
        server
    }
}

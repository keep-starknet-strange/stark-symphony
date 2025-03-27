use elements::{encode, Transaction};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::env;

/// Broadcast a transaction to the network
/// Returns the txid of the broadcasted transaction
#[allow(dead_code)]
pub fn broadcast_tx(tx: Transaction) -> anyhow::Result<String> {
    let rpc_url = env::var("RPC_URL").map_err(|_| anyhow::anyhow!("RPC_URL is not set"))?;
    let rpc_user = env::var("RPC_USER").map_err(|_| anyhow::anyhow!("RPC_USER is not set"))?;
    let rpc_password =
        env::var("RPC_PASSWORD").map_err(|_| anyhow::anyhow!("RPC_PASSWORD is not set"))?;

    let tx_hex = encode::serialize_hex(&tx);
    let request = json!({
        "jsonrpc": "1.0",
        "method": "sendrawtransaction",
        "params": [tx_hex],
    });

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&rpc_url)
        .basic_auth(rpc_user, Some(rpc_password))
        .header("Content-Type", "application/json")
        .body(request.to_string())
        .send()?;

    let status = res.status();
    if status != StatusCode::OK {
        return Err(anyhow::anyhow!(
            "Failed to broadcast transaction: {}",
            res.text().unwrap_or(status.to_string())
        ));
    }

    let response_text = res.text()?;
    let response: Value = serde_json::from_str(&response_text)?;
    let txid = response["result"].as_str().unwrap_or_default().to_string();

    Ok(txid)
}

use maud::html;
use serde::Serialize;
use std::collections::HashMap;

use crate::primitives::{SlpAction, TokenMeta};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonUtxo {
    pub tx_hash: String,
    pub out_idx: u32,
    pub sats_amount: i64,
    pub token_amount: u64,
    pub is_coinbase: bool,
    pub block_height: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonBalance {
    pub token_idx: Option<usize>,
    pub sats_amount: i64,
    pub token_amount: u64,
    pub utxos: Vec<JsonUtxo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonToken {
    pub token_id: String,
    pub token_type: u32,
    pub token_ticker: String,
    pub token_name: String,
    pub decimals: u32,
    pub group_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonTx {
    pub tx_hash: String,
    pub block_height: Option<i32>,
    pub timestamp: i64,
    pub is_coinbase: bool,
    pub size: i32,
    pub num_inputs: u32,
    pub num_outputs: u32,
    pub sats_input: i64,
    pub sats_output: i64,
    pub delta_sats: i64,
    pub delta_tokens: i64,
    pub token_idx: Option<usize>,
    pub is_burned_slp: bool,
    pub token_input: u64,
    pub token_output: u64,
    pub slp_action: Option<SlpAction>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonTxs {
    pub txs: Vec<JsonTx>,
    pub tokens: Vec<JsonToken>,
    pub token_indices: HashMap<Vec<u8>, usize>,
}

impl JsonToken {
    pub fn from_token_meta(token_id: &[u8], token_meta: TokenMeta) -> Self {
        let token_ticker = String::from_utf8_lossy(&token_meta.token_ticker);
        let token_name = String::from_utf8_lossy(&token_meta.token_name);
        JsonToken {
            token_id: hex::encode(token_id),
            token_type: token_meta.token_type,
            token_ticker: html! { (token_ticker) }.into_string(),
            token_name: html! { (token_name) }.into_string(),
            decimals: token_meta.decimals,
            group_id: token_meta.group_id.map(|group_id| hex::encode(&group_id)),
        }
    }
}

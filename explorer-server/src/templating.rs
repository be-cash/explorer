use askama::Template;
use bitcoinsuite_chronik_client::proto;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::{blockchain::Destination, server_primitives::{JsonBalance, JsonSlpv2Section}};

pub mod filters;

#[derive(Template)]
#[template(path = "pages/homepage.html")]
pub struct HomepageTemplate {}

#[derive(Template)]
#[template(path = "pages/blocks.html")]
pub struct BlocksTemplate {
    pub last_block_height: u32,
}

#[derive(Template)]
#[template(path = "pages/block.html")]
pub struct BlockTemplate<'a> {
    pub block_hex: &'a str,
    pub block_header: Vec<u8>,
    pub block_info: proto::BlockInfo,
    pub confirmations: i32,
    pub timestamp: DateTime<chrono::Utc>,
    pub difficulty: f64,
    pub coinbase_data: Vec<u8>,
}

pub struct TemplateSlpv2TokenSection {
    pub section_type: String,
    pub data: JsonSlpv2Section,
}

#[derive(Template)]
#[template(path = "pages/transaction.html")]
pub struct TransactionTemplate<'a> {
    pub title: &'a str,
    pub is_token: bool,
    pub tx_hex: &'a str,
    pub slpv2_sections: Vec<TemplateSlpv2TokenSection>,
    pub tx: proto::Tx,
    pub raw_tx: String,
    pub confirmations: i32,
    pub timestamp: DateTime<Utc>,
    pub sats_input: i64,
    pub sats_output: i64,
}

#[derive(Template)]
#[template(path = "pages/address.html")]
pub struct AddressTemplate<'a> {
    pub tokens: HashMap<String, proto::Slpv2TokenInfo>,
    pub token_dust: i64,
    pub total_xec: i64,
    pub token_utxos: Vec<proto::ScriptUtxo>,
    pub address_num_txs: u32,
    pub address: &'a str,
    pub sats_address: &'a str,
    pub token_address: &'a str,
    pub legacy_address: String,
    pub json_balances: HashMap<String, JsonBalance>,
    pub encoded_tokens: String,
    pub encoded_balances: String,
}

#[derive(Template)]
#[template(path = "pages/error.html")]
pub struct ErrorTemplate {
    pub message: String,
}

use chrono::{DateTime, Utc};
use askama::Template;
use bitcoin_cash::Address;

use crate::{blockchain::{BlockHeader, Destination}, primitives::{BlockMeta, TxMetaVariant}, indexer::Tx, server_primitives::{JsonBalance, JsonTxs}};

mod filters;

#[derive(Template)]
#[template(path = "pages/homepage.html")]
pub struct HomepageTemplate {
}

#[derive(Template)]
#[template(path = "pages/blocks.html")]
pub struct BlocksTemplate {
    pub last_block_height: u32,
}

#[derive(Template)]
#[template(path = "pages/block.html")]
pub struct BlockTemplate<'a> {
    pub block_hash_string: &'a str,
    pub block_header: BlockHeader,
    pub block_meta: BlockMeta,
    pub confirmations: u32,
    pub timestamp: DateTime<chrono::Utc>,
}

#[derive(Template)]
#[template(path = "pages/transaction.html")]
pub struct TransactionTemplate<'a> {
    pub title: &'a str,
    pub token_section_title: &'a str,
    pub is_token: bool,
    pub tx_hash_string: &'a str,
    pub token_hash_string: Option<String>,
    pub tx: Tx,
    pub block_meta: Option<BlockMeta>,
    pub confirmations: u32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Template)]
#[template(path = "pages/address.html")]
pub struct AddressTemplate<'a> {
    pub cash_balance: JsonBalance,
    pub token_dust: i64,
    pub address_num_txs: usize,
    pub sats_address: &'a Address<'a>,
    pub token_address: &'a Address<'a>,
    pub legacy_address: String,
}

use anyhow::{Result, anyhow, bail};
use bitcoin_cash::Address;
use maud::html;
use warp::{Reply, http::Uri};
use serde::Serialize;
use chrono::{Utc, TimeZone};
use std::{borrow::Cow, collections::{BTreeSet, HashMap, hash_map::Entry}, convert::{TryInto, TryFrom}, sync::Arc};
use zerocopy::byteorder::{I32, U32};
use askama::Template;

use crate::{
    blockchain::{BlockHeader, from_le_hex, to_legacy_address, to_le_hex},
    indexdb::AddressBalance,
    indexer::Indexer,
    primitives::{SlpAction, TxMeta, TxMetaVariant},
    server_primitives::{JsonUtxo, JsonBalance, JsonToken, JsonTx, JsonTxs },
    templating::{HomepageTemplate, BlocksTemplate, BlockTemplate, TransactionTemplate, AddressTemplate},
};

pub struct Server {
    indexer: Arc<dyn Indexer>,
    satoshi_addr_prefix: &'static str,
    tokens_addr_prefix: &'static str,
}

impl Server {
    pub async fn setup(indexer: Arc<dyn Indexer>) -> Result<Self> {
        Ok(Server {
            indexer,
            satoshi_addr_prefix: "ecash",
            tokens_addr_prefix: "etoken",
        })
    }
}

impl Server {
    pub async fn homepage(&self) -> Result<impl Reply> {
        let homepage = HomepageTemplate {  };
        Ok(warp::reply::html(homepage.render().unwrap()))
    }

    fn generate_pagination(&self, page: usize, last_page: usize, curated_page_offsets: &[usize]) -> BTreeSet<usize> {
        let mut pages = BTreeSet::new();

        pages.insert(0);
        pages.insert(page);
        pages.insert(last_page);

        for &page_offset in curated_page_offsets.iter().rev() {
            let preceding_page = page.saturating_sub(page_offset) / page_offset * page_offset;
            if preceding_page > 0 {
                pages.insert(preceding_page);
            }
        }

        for &page_offset in curated_page_offsets.iter() {
            let following_page = page.saturating_add(page_offset) / page_offset * page_offset;
            if following_page >= last_page {
                pages.insert(last_page);
                break;
            }
            pages.insert(following_page);
        }

        pages
    }

    pub async fn blocks(&self, query: HashMap<String, String>) -> Result<impl Reply> {
        let half_page_size = 500;
        let page_size = half_page_size * 2;
        let best_height = self.indexer.db().last_block_height()?;
        let page = query.get("page").and_then(|page| page.parse().ok()).unwrap_or(0u32);
        let half_page = page * 2;
        let best_page_height = (best_height / half_page_size) * half_page_size;
        let first_page_begin = best_page_height.saturating_sub(half_page * half_page_size);
        let first_page_end = (first_page_begin + half_page_size - 1).min(best_height);
        let second_page_begin = first_page_begin.saturating_sub(half_page_size);
        let second_page_end = first_page_begin.saturating_sub(1);
        let last_page = best_height / page_size;
        let curated_page_offsets = &[
            1, 2, 3, 10, 20, 50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 1000,
        ];
        let pages = self.generate_pagination(page as usize, last_page as usize, curated_page_offsets);

        let blocks_template = BlocksTemplate {
            query_string: "?page=",
            pages: pages,
            first_page_begin: first_page_begin,
            first_page_end: first_page_end,
            second_page_begin: second_page_begin,
            second_page_end: second_page_end,
            last_block_height: best_height,
        };
        Ok(warp::reply::html(blocks_template.render().unwrap()))
    }

    pub async fn data_blocks(&self, start_height: u32, end_height: u32) -> Result<impl Reply> {
        let num_blocks = end_height.checked_sub(start_height).unwrap() + 1;
        let blocks = self.indexer.db().block_range(start_height, num_blocks)?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Block {
            hash: String,
            height: i32,

            version: i32,
            timestamp: i64,

            difficulty: f64,
            size: u64,
            num_txs: u64,
            median_time: i64,
        }
        let mut json_blocks = Vec::with_capacity(blocks.len());
        for (block_hash, block) in blocks.into_iter().rev() {
            json_blocks.push(Block {
                hash: to_le_hex(&block_hash),
                height: block.height,
                version: block.version,
                timestamp: block.timestamp,
                difficulty: block.difficulty,
                size: block.size,
                median_time: block.median_time,
                num_txs: block.num_txs,
            });
        }

        Ok(serde_json::to_string(&json_blocks)?)
    }
}

impl Server {
    pub async fn data_block_txs(&self, block_hash: &str) -> Result<impl Reply> {
        let block_hash = from_le_hex(block_hash)?;
        let block_txs = self.indexer.block_txs(&block_hash).await?;
        let json_txs = self.json_txs(
            block_txs.iter()
                .map(|(tx_hash, tx_meta)| (tx_hash.as_ref(), 0, Some(tx_meta.block_height), tx_meta, (0, 0)))
        ).await?;
        let encoded_txs = serde_json::to_string(&json_txs.txs)?;
        let encoded_tokens = serde_json::to_string(&json_txs.tokens)?;
        let reply = format!(r#"
            if (window.txData === undefined)
                window.txData = [];
            {{
                var txs = JSON.parse('{encoded_txs}');
                var tokens = JSON.parse('{encoded_tokens}');
                var startIdx = window.txData.length;
                window.txData.length += txs.length;
                for (var i = 0; i < txs.length; ++i) {{
                    var tx = txs[i];
                    tx.token = tx.tokenIdx === null ? null : tokens[tx.tokenIdx];
                    window.txData[startIdx + i] = tx;
                }}
            }}
        "#, encoded_txs = encoded_txs, encoded_tokens = encoded_tokens);
        let reply = warp::reply::with_header(reply, "content-type", "application/javascript");
        let reply = warp::reply::with_header(reply, "last-modified", "Tue, 29 Dec 2020 06:31:27 GMT");
        Ok(reply)
    }

    async fn json_txs(&self, txs: impl ExactSizeIterator<Item=(&[u8], i64, Option<i32>, &TxMeta, (i64, i64))>) -> Result<JsonTxs> {
        let mut json_txs = Vec::with_capacity(txs.len());
        let mut token_indices = HashMap::<Vec<u8>, usize>::new();
        for (tx_hash, timestamp, block_height, tx_meta, (delta_sats, delta_tokens)) in txs {
            let mut tx = JsonTx {
                tx_hash: to_le_hex(&tx_hash),
                block_height,
                timestamp,
                is_coinbase: tx_meta.is_coinbase,
                size: tx_meta.size,
                num_inputs: tx_meta.num_inputs,
                num_outputs: tx_meta.num_outputs,
                sats_input: tx_meta.sats_input,
                sats_output: tx_meta.sats_output,
                delta_sats,
                delta_tokens,
                token_idx: None,
                is_burned_slp: false,
                token_input: 0,
                token_output: 0,
                slp_action: None,
            };
            let mut tx_token_id = None;
            match &tx_meta.variant {
                TxMetaVariant::SatsOnly => {},
                TxMetaVariant::InvalidSlp { token_id, token_input } => {
                    tx_token_id = Some(token_id.clone());
                    tx.is_burned_slp = true;
                    tx.token_input = *token_input;
                }
                TxMetaVariant::Slp { token_id, token_input, token_output, action } => {
                    tx_token_id = Some(token_id.to_vec());
                    tx.token_input = *token_input;
                    tx.token_output = *token_output;
                    tx.slp_action = Some(*action);
                }
            }
            if let Some(token_id) = tx_token_id {
                if token_id.len() == 32 {
                    let num_tokens = token_indices.len();
                    match token_indices.entry(token_id) {
                        Entry::Vacant(vacant) => {
                            vacant.insert(num_tokens);
                            tx.token_idx = Some(num_tokens);
                        },
                        Entry::Occupied(occupied) => {
                            tx.token_idx = Some(*occupied.get());
                        }
                    }
                }
            }
            json_txs.push(tx);
        }
        let tokens = token_indices
            .keys()
            .map(|key| self.indexer.db().token_meta(key))
            .collect::<Result<Vec<_>, _>>()?;
        let mut token_data = tokens.into_iter().zip(&token_indices).collect::<Vec<_>>();
        token_data.sort_unstable_by_key(|&(_, (_, idx))| idx);
        let json_tokens = token_data.into_iter().filter_map(|(token_meta, (token_id, _))| {
            Some(JsonToken::from_token_meta(token_id, token_meta?))
        }).collect::<Vec<_>>();
        Ok(JsonTxs { tokens: json_tokens, txs: json_txs, token_indices })
    }

    pub async fn block(&self, block_hash_str: &str) -> Result<impl Reply> {
        let block_hash = from_le_hex(block_hash_str)?;
        let block_meta = self.indexer.db().block_meta(&block_hash)?.ok_or_else(|| anyhow!("No such block"))?;
        let best_height = self.indexer.db().last_block_height()?;
        let confirmations = best_height - block_meta.height as u32 + 1;
        let timestamp = Utc.timestamp(block_meta.timestamp, 0);
        let mut block_header = BlockHeader::default();
        block_header.version = I32::new(block_meta.version);
        block_header.previous_block = block_meta.previous_block;
        block_header.merkle_root = block_meta.merkle_root;
        block_header.timestamp = U32::new(block_meta.timestamp.try_into()?);
        block_header.bits = U32::new(block_meta.bits);
        block_header.nonce = U32::new(block_meta.nonce);

        let block_template = BlockTemplate {
            block_hash_string: block_hash_str,
            block_header: block_header,
            block_meta: block_meta,
            confirmations: confirmations,
            timestamp: timestamp,
        };
        
        Ok(warp::reply::html(block_template.render().unwrap()))
    }

    pub async fn tx(&self, tx_hash_str: &str) -> Result<impl Reply> {
        use SlpAction::*;

        let tx_hash = from_le_hex(tx_hash_str)?;
        let tx = self.indexer.tx(&tx_hash).await?;
        let title: Cow<str> = match tx.tx_meta.variant {
            TxMetaVariant::SatsOnly => "eCash Transaction".into(),
            TxMetaVariant::InvalidSlp {..} => "Invalid eToken Transaction".into(),
            TxMetaVariant::Slp {..} => {
                let token_meta = tx.token_meta.as_ref().ok_or_else(|| anyhow!("No token meta"))?;
                format!("{} Token Transaction", String::from_utf8_lossy(&token_meta.token_ticker)).into()
            }
        };
        let token_hash_str = match tx.tx_meta.variant {
            TxMetaVariant::SatsOnly => None,
            TxMetaVariant::Slp { token_id, .. } => Some(hex::encode(&token_id)),
            TxMetaVariant::InvalidSlp { ref token_id, .. } => Some(hex::encode(&token_id))
        };
        let token_section_title = match (&tx.tx_meta.variant, &tx.token_meta) {
            (
                TxMetaVariant::Slp { action, .. },
                Some(_),
            ) => {
                let action_str = match action {
                    SlpV1Genesis => "GENESIS",
                    SlpV1Mint => "MINT",
                    SlpV1Send => "SEND",
                    SlpV1Nft1GroupGenesis => "NFT1 Group GENESIS",
                    SlpV1Nft1GroupMint => "NFT1 MINT",
                    SlpV1Nft1GroupSend => "NFT1 Group SEND",
                    SlpV1Nft1UniqueChildGenesis => "NFT1 Child GENESIS",
                    SlpV1Nft1UniqueChildSend => "NFT1 Child SEND",
                };
                format!("Token Details ({} Transaction)", action_str)
            },
            (TxMetaVariant::InvalidSlp { .. }, Some(_)) => String::from("Token Details (Invalid Transaction)"),
            (TxMetaVariant::InvalidSlp { .. }, None) => String::from("Token Details (Invalid Transaction; Unknown Token)"),
            _ => String::from(""),
        };
        let is_token = if &title == "eCash Transaction" { false } else { true };
        let block_meta = self.indexer.db().block_meta(&tx.transaction.block_hash)?;
        let best_height = self.indexer.db().last_block_height()?;
        let confirmations = match &block_meta {
            Some(block_meta) => best_height - block_meta.height as u32 + 1,
            None => 0,
        };
        let timestamp = Utc.timestamp(tx.transaction.timestamp, 0);

        let transaction_template = TransactionTemplate {
            title: title.as_ref(),
            token_section_title: &token_section_title,
            is_token: is_token,
            tx_hash_string: tx_hash_str,
            token_hash_string: token_hash_str,
            tx: tx,
            block_meta: block_meta,
            confirmations: confirmations,
            timestamp: timestamp,
        };
        Ok(warp::reply::html(transaction_template.render().unwrap()))
    }
}

impl Server {
    pub async fn address(&self, address: &str, query: HashMap<String, String>) -> Result<impl Reply> {
        let address = Address::from_cash_addr(address)?;
        let txs_page: usize = query.get("tx_page").map(|s| s.as_str()).unwrap_or("0").parse()?;
        let coins_page: usize = query.get("coin_page").map(|s| s.as_str()).unwrap_or("0").parse()?;
        let page_size = 500;
        let sats_address = address.with_prefix(self.satoshi_addr_prefix);
        let token_address = address.with_prefix(self.tokens_addr_prefix);
        let legacy_address = to_legacy_address(&address);
        let address_txs = self.indexer.db().address(&sats_address, txs_page * page_size, page_size)?;
        let address_num_txs = self.indexer.db().address_num_txs(&address)?;
        let mut json_txs = self.json_txs(
            address_txs
                .iter()
                .map(|(tx_hash, addr_tx, tx_meta)| {
                    (tx_hash.as_ref(), addr_tx.timestamp, Some(addr_tx.block_height), tx_meta, (addr_tx.delta_sats, addr_tx.delta_tokens))
                })
        ).await?;
        let balance = self.indexer.db().address_balance(&sats_address, coins_page * page_size, page_size)?;
        let AddressBalance { balances, utxos } = balance;
        for (token_id, _) in &utxos {
            if let Some(token_id) = &token_id {
                if !json_txs.token_indices.contains_key(token_id.as_ref()) {
                    if let Some(token_meta) = self.indexer.db().token_meta(token_id)? {
                        json_txs.token_indices.insert(token_id.to_vec(), json_txs.tokens.len());
                        json_txs.tokens.push(JsonToken::from_token_meta(token_id, token_meta));
                    }
                }
            }
        }
        let token_dust = balances.iter()
            .filter_map(|(token_id, balance)| token_id.and(Some(balance.0)))
            .sum::<i64>();
        let mut json_balances = utxos.into_iter().map(|(token_id, mut utxos)| {
            let (sats_amount, token_amount) = balances[&token_id];
            utxos.sort_by_key(|(_, utxo)| -utxo.block_height);
            (
                utxos.get(0).map(|(_, utxo)| utxo.block_height).unwrap_or(0),
                JsonBalance {
                    token_idx: token_id.and_then(|token_id| json_txs.token_indices.get(token_id.as_ref())).copied(),
                    sats_amount,
                    token_amount,
                    utxos: utxos.into_iter().map(|(utxo_key, utxo)| JsonUtxo {
                        tx_hash: to_le_hex(&utxo_key.tx_hash),
                        out_idx: utxo_key.out_idx.get(),
                        sats_amount: utxo.sats_amount,
                        token_amount: utxo.token_amount,
                        is_coinbase: utxo.is_coinbase,
                        block_height: utxo.block_height,
                    }).collect(),
                }
            )
        }).collect::<Vec<_>>();
        json_balances.sort_by_key(|(block_height, balance)| {
            if balance.token_idx.is_none() {
                i32::MIN
            } else {
                -block_height
            }
        });
        let json_balances: Vec<JsonBalance> = json_balances.into_iter().map(|(_, balance)| balance).collect::<Vec<_>>();

        let encoded_txs = serde_json::to_string(&json_txs.txs)?.replace("'", r"\'");
        let encoded_tokens = serde_json::to_string(&json_txs.tokens)?.replace("'", r"\'");
        let encoded_balances = serde_json::to_string(&json_balances)?.replace("'", r"\'");

        let address_template = AddressTemplate {
            json_balances: json_balances,
            token_dust: token_dust,
            address_num_txs: address_num_txs,
            json_txs: json_txs,
            address: &address,
            sats_address: &sats_address,
            token_address: &token_address,
            legacy_address: legacy_address,
            encoded_txs: encoded_txs,
            encoded_tokens: encoded_tokens,
            encoded_balances: encoded_balances,
        };
        Ok(warp::reply::html(address_template.render().unwrap()))
    }

    pub async fn address_qr(&self, address: &str) -> Result<impl Reply> {
        use qrcode_generator::QrCodeEcc;
        if address.len() > 60 {
            bail!("Invalid address length");
        }
        let png = qrcode_generator::to_png_to_vec(address, QrCodeEcc::Quartile, 160)?;
        let reply = warp::reply::with_header(png, "Content-Type", "image/png");
        Ok(reply)
    }

    pub async fn block_height(&self, height: u32) -> Result<Box<dyn Reply>> {
        let block_hash = self.indexer.db().block_hash_at(height)?;
        match block_hash {
            Some(block_hash) => {
                let block_hash_str = to_le_hex(&block_hash);
                let url = format!("/block/{}", block_hash_str);
                Ok(Box::new(warp::redirect(Uri::try_from(url.as_str())?)))
            },
            None => Ok(Box::new(warp::reply::html(html! {
                h1 { "Not found" }
            }.into_string())))
        }
    }

    pub async fn search(&self, query: &str) -> Result<Box<dyn Reply>> {
        match self.indexer.db().search(query)? {
            Some(url) => Ok(Box::new(warp::redirect(Uri::try_from(url.as_str())?))),
            None => Ok(Box::new(warp::reply::html(html! {
                h1 { "Not found" }
            }.into_string())))
        }
    }
}

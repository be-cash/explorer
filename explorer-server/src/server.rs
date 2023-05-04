use askama::Template;
use axum::{response::Redirect, routing::get, Router};
use bitcoinsuite_chronik_client::proto::{self};
use bitcoinsuite_chronik_client::{proto::OutPoint, ChronikClient};
use bitcoinsuite_core::{CashAddress, Hashed, Sha256d};
use bitcoinsuite_error::Result;
use chrono::{TimeZone, Utc};
use eyre::{bail, eyre};
use futures::future;
use std::path::PathBuf;
use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap, HashSet},
};

use crate::api::calc_section_stats;
use crate::server_primitives::{JsonSlpv2Section, JsonSlpv2TokenInfo};
use crate::templating::TemplateSlpv2TokenSection;
use crate::{
    api::{block_txs_to_json, calc_tx_stats, tokens_to_json, tx_history_to_json},
    blockchain::{
        calculate_block_difficulty, cash_addr_to_script_type_payload, from_be_hex, to_be_hex,
        to_legacy_address,
    },
    server_http::{
        address, address_qr, block, block_height, blocks, data_address_txs, data_block_txs,
        data_blocks, homepage, search, serve_files, tx,
    },
    server_primitives::{JsonBalance, JsonBlock, JsonBlocksResponse, JsonTxsResponse, JsonUtxo},
    templating::{
        AddressTemplate, BlockTemplate, BlocksTemplate, HomepageTemplate, TransactionTemplate,
    },
};

pub struct Server {
    chronik: ChronikClient,
    base_dir: PathBuf,
    satoshi_addr_prefix: &'static str,
    tokens_addr_prefix: &'static str,
}

impl Server {
    pub async fn setup(chronik: ChronikClient, base_dir: PathBuf) -> Result<Self> {
        Ok(Server {
            chronik,
            base_dir,
            satoshi_addr_prefix: "ecash",
            tokens_addr_prefix: "etoken",
        })
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/", get(homepage))
            .route("/tx/:hash", get(tx))
            .route("/blocks", get(blocks))
            .route("/block/:hash", get(block))
            .route("/block-height/:height", get(block_height))
            .route("/address/:hash", get(address))
            .route("/address-qr/:hash", get(address_qr))
            .route("/search/:query", get(search))
            .route("/api/blocks/:start_height/:end_height", get(data_blocks))
            .route("/api/block/:hash/transactions", get(data_block_txs))
            .route("/api/address/:hash/transactions", get(data_address_txs))
            .nest("/code", serve_files(&self.base_dir.join("code")))
            .nest("/assets", serve_files(&self.base_dir.join("assets")))
            .nest(
                "/favicon.ico",
                serve_files(&self.base_dir.join("assets").join("favicon.png")),
            )
    }
}

impl Server {
    pub async fn homepage(&self) -> Result<String> {
        let homepage = HomepageTemplate {};
        Ok(homepage.render().unwrap())
    }

    pub async fn blocks(&self) -> Result<String> {
        let blockchain_info = self.chronik.blockchain_info().await?;

        let blocks_template = BlocksTemplate {
            last_block_height: blockchain_info.tip_height as u32,
        };

        Ok(blocks_template.render().unwrap())
    }
}

impl Server {
    pub async fn data_blocks(
        &self,
        start_height: i32,
        end_height: i32,
    ) -> Result<JsonBlocksResponse> {
        let blocks = self.chronik.blocks(start_height, end_height).await?;

        let mut json_blocks = Vec::with_capacity(blocks.len());
        for block in blocks.into_iter().rev() {
            json_blocks.push(JsonBlock {
                hash: to_be_hex(&block.hash),
                height: block.height,
                timestamp: block.timestamp,
                difficulty: calculate_block_difficulty(block.n_bits),
                size: block.block_size,
                num_txs: block.num_txs,
            });
        }

        Ok(JsonBlocksResponse { data: json_blocks })
    }

    pub async fn data_block_txs(&self, block_hex: &str) -> Result<JsonTxsResponse> {
        let block_hash = Sha256d::from_hex_be(block_hex)?;
        let block = self.chronik.block_by_hash(&block_hash).await?;

        let mut block_txs = Vec::new();
        let mut token_ids = HashSet::new();
        let mut page = 0;
        loop {
            let page_txs = self
                .chronik
                .block_txs_by_hash(&block_hash, page, 200)
                .await?;
            for tx in &page_txs.txs {
                for section in &tx.slpv2_sections {
                    token_ids.insert(Sha256d::from_slice(&section.token_id)?);
                }
                for burn_token_id in &tx.slpv2_burn_token_ids {
                    token_ids.insert(Sha256d::from_slice(burn_token_id)?);
                }
            }
            block_txs.extend(page_txs.txs);
            page += 1;
            if page == page_txs.num_pages as usize {
                break;
            }
        }

        let tokens_by_hex = self.batch_get_chronik_tokens(token_ids).await?;
        let json_txs = block_txs_to_json(block, &block_txs, &tokens_by_hex)?;

        Ok(JsonTxsResponse { data: json_txs })
    }

    pub async fn data_address_txs(
        &self,
        address: &str,
        query: HashMap<String, String>,
    ) -> Result<JsonTxsResponse> {
        let address = CashAddress::parse_cow(address.into())?;
        let (script_type, script_payload) = cash_addr_to_script_type_payload(&address);
        let script_endpoint = self.chronik.script(script_type, &script_payload);

        let page: usize = query
            .get("page")
            .map(|s| s.as_str())
            .unwrap_or("0")
            .parse()?;
        let take: usize = query
            .get("take")
            .map(|s| s.as_str())
            .unwrap_or("200")
            .parse()?;
        let address_tx_history = script_endpoint.history_with_page_size(page, take).await?;

        let mut token_ids = HashSet::new();
        for tx in &address_tx_history.txs {
            for section in &tx.slpv2_sections {
                token_ids.insert(Sha256d::from_slice(&section.token_id)?);
            }
        }

        let tokens = self.batch_get_chronik_tokens(token_ids).await?;
        let json_tokens = tokens_to_json(&tokens)?;
        let json_txs = tx_history_to_json(&address, address_tx_history, &json_tokens)?;

        Ok(JsonTxsResponse { data: json_txs })
    }
}

impl Server {
    pub async fn block(&self, block_hex: &str) -> Result<String> {
        let block_hash = Sha256d::from_hex_be(block_hex)?;

        let block = self.chronik.block_by_hash(&block_hash).await?;
        let block_info = block.block_info.ok_or_else(|| eyre!("Block has no info"))?;
        /*let block_details = block
        .block_details
        .ok_or_else(|| eyre!("Block has details"))?;*/

        let blockchain_info = self.chronik.blockchain_info().await?;
        let best_height = blockchain_info.tip_height;

        let difficulty = calculate_block_difficulty(block_info.n_bits);
        let timestamp = Utc.timestamp(block_info.timestamp, 0);
        //let coinbase_data = block.txs[0].inputs[0].input_script.clone();
        let confirmations = best_height - block_info.height + 1;

        let block_template = BlockTemplate {
            block_hex,
            block_header: vec![], // TODO
            block_info,
            confirmations,
            timestamp,
            difficulty,
            coinbase_data: vec![], // TODO
        };

        Ok(block_template.render().unwrap())
    }

    pub async fn tx(&self, tx_hex: &str) -> Result<String> {
        let tx_hash = Sha256d::from_hex_be(tx_hex)?;
        let tx = self.chronik.tx(&tx_hash).await?;

        let mut slpv2_sections = Vec::new();
        for section in &tx.slpv2_sections {
            let token_id = Sha256d::from_slice(&section.token_id)?;
            let token_info = self.chronik.token(&token_id).await?;
            let genesis_data = token_info.genesis_data.expect("Missing genesis_data");
            let token_ticker = String::from_utf8_lossy(&genesis_data.token_ticker);
            let token_name = String::from_utf8_lossy(&genesis_data.token_name);
            let token_url = String::from_utf8_lossy(&genesis_data.url);
            let section_type = match (section.token_type(), section.section_type()) {
                (proto::Slpv2TokenType::Standard, proto::Slpv2SectionType::Slpv2Genesis) => {
                    "GENESIS"
                }
                (proto::Slpv2TokenType::Standard, proto::Slpv2SectionType::Slpv2Send) => "SEND",
                (proto::Slpv2TokenType::Standard, proto::Slpv2SectionType::Slpv2Mint) => "MINT",
                _ => "Unknown",
            };
            slpv2_sections.push(TemplateSlpv2TokenSection {
                section_type: section_type.to_string(),
                data: JsonSlpv2Section {
                    token_info: JsonSlpv2TokenInfo {
                        token_id: token_id.to_string(),
                        token_type: section.token_type as u32,
                        token_ticker: token_ticker.to_string(),
                        token_name: token_name.to_string(),
                        token_url: token_url.to_string(),
                        decimals: genesis_data.decimals,
                        token_color: crate::templating::filters::to_token_color(token_id.as_slice()).unwrap(),
                    },
                    stats: calc_section_stats(&tx, section, None),
                },
            });
        }

        let (title, is_token): (Cow<str>, bool) = if slpv2_sections.is_empty() {
            if tx.slpv2_errors.is_empty() {
                ("eCash Transaction".into(), false)
            } else {
                ("Invalid eToken Transaction".into(), true)
            }
        } else {
            let mut title = String::new();
            for (idx, section) in slpv2_sections.iter().enumerate() {
                if idx > 0 {
                    if idx == slpv2_sections.len() - 1 {
                        title.push_str(" & ");
                    } else {
                        title.push_str(", ");
                    }
                }
                title.push_str(&section.data.token_info.token_ticker);
            }
            title.push_str(" Transaction");
            (title.into(), true)
        };

        let blockchain_info = self.chronik.blockchain_info().await?;
        let confirmations = match &tx.block {
            Some(block_meta) => blockchain_info.tip_height - block_meta.height + 1,
            None => 0,
        };
        let timestamp = match &tx.block {
            Some(block_meta) => Utc.timestamp(block_meta.timestamp, 0),
            None => Utc.timestamp(tx.time_first_seen, 0),
        };

        let raw_tx = self.chronik.raw_tx(&tx_hash).await?;
        let raw_tx = raw_tx.hex();

        let tx_stats = calc_tx_stats(&tx, None);

        let transaction_template = TransactionTemplate {
            title: &title,
            is_token,
            tx_hex,
            slpv2_sections,
            tx,
            sats_input: tx_stats.sats_input,
            sats_output: tx_stats.sats_output,
            raw_tx,
            confirmations,
            timestamp,
        };

        Ok(transaction_template.render().unwrap())
    }
}

impl Server {
    pub async fn address<'a>(&'a self, address: &str) -> Result<String> {
        let address = CashAddress::parse_cow(address.into())?;
        let sats_address = address.with_prefix(self.satoshi_addr_prefix);
        let token_address = address.with_prefix(self.tokens_addr_prefix);

        let legacy_address = to_legacy_address(&address);
        let sats_address = sats_address.as_str();
        let token_address = token_address.as_str();

        let (script_type, script_payload) = cash_addr_to_script_type_payload(&address);
        let script_endpoint = self.chronik.script(script_type, &script_payload);
        let page_size = 1; // Set to minimum so that num_pages == total existing tx's
        let address_tx_history = script_endpoint.history_with_page_size(0, page_size).await?;
        let address_num_txs = address_tx_history.num_pages;

        let utxos = script_endpoint.utxos().await?;

        let mut token_dust: i64 = 0;
        let mut total_xec: i64 = 0;

        let mut token_ids: HashSet<Sha256d> = HashSet::new();
        let mut token_utxos: Vec<proto::ScriptUtxo> = Vec::new();
        let mut json_balances: HashMap<String, JsonBalance> = HashMap::new();
        let mut main_json_balance: JsonBalance = JsonBalance {
            token_id: None,
            sats_amount: 0,
            token_amount: 0,
            utxos: Vec::new(),
        };

        for utxo in utxos.utxos.into_iter() {
            let OutPoint { txid, out_idx } = &utxo.outpoint.as_ref().unwrap();
            let mut json_utxo = JsonUtxo {
                tx_hash: to_be_hex(txid),
                out_idx: *out_idx,
                sats_amount: utxo.value,
                token_amount: 0,
                is_coinbase: utxo.is_coinbase,
                block_height: utxo.block_height,
                is_mint_baton: false,
            };

            match &utxo.slpv2 {
                Some(token) => {
                    let token_id = Sha256d::from_slice(&token.token_id)?;

                    json_utxo.token_amount = token.amount as u64;
                    json_utxo.is_mint_baton = token.is_mint_baton;

                    match json_balances.entry(token_id.to_string()) {
                        Entry::Occupied(mut entry) => {
                            let entry = entry.get_mut();
                            entry.sats_amount += utxo.value;
                            entry.token_amount += token.amount;
                            entry.utxos.push(json_utxo);
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(JsonBalance {
                                token_id: Some(token_id.to_string()),
                                sats_amount: utxo.value,
                                token_amount: token.amount.into(),
                                utxos: vec![json_utxo],
                            });
                        }
                    }

                    token_ids.insert(token_id);
                    token_dust += utxo.value;
                    token_utxos.push(utxo);
                }
                _ => {
                    total_xec += utxo.value;
                    main_json_balance.utxos.push(json_utxo);
                }
            };
        }
        json_balances.insert(String::from("main"), main_json_balance);

        let tokens = self.batch_get_chronik_tokens(token_ids).await?;
        let json_tokens = tokens_to_json(&tokens)?;

        let encoded_tokens = serde_json::to_string(&json_tokens)?.replace('\'', r"\'");
        let encoded_balances = serde_json::to_string(&json_balances)?.replace('\'', r"\'");

        let address_template = AddressTemplate {
            tokens,
            token_utxos,
            token_dust,
            total_xec,
            address_num_txs,
            address: address.as_str(),
            sats_address,
            token_address,
            legacy_address,
            json_balances,
            encoded_tokens,
            encoded_balances,
        };

        Ok(address_template.render().unwrap())
    }

    pub async fn batch_get_chronik_tokens(
        &self,
        token_ids: HashSet<Sha256d>,
    ) -> Result<HashMap<String, proto::Slpv2TokenInfo>> {
        let mut token_calls = Vec::new();
        let mut token_map = HashMap::new();

        for token_id in token_ids.iter() {
            token_calls.push(Box::pin(self.chronik.token(token_id)));
        }

        let tokens = future::try_join_all(token_calls).await?;
        for token in tokens.into_iter() {
            token_map.insert(Sha256d::from_slice(&token.token_id)?.to_string(), token);
        }

        Ok(token_map)
    }

    pub async fn address_qr(&self, address: &str) -> Result<Vec<u8>> {
        use qrcode_generator::QrCodeEcc;
        if address.len() > 60 {
            bail!("Invalid address length");
        }
        let png = qrcode_generator::to_png_to_vec(address, QrCodeEcc::Quartile, 160)?;
        Ok(png)
    }

    pub async fn block_height(&self, height: u32) -> Result<Redirect> {
        let block = self.chronik.block_by_height(height as i32).await.ok();

        match block {
            Some(block) => {
                let block_info = block.block_info.expect("Impossible");
                Ok(self.redirect(format!("/block/{}", to_be_hex(&block_info.hash))))
            }
            None => Ok(self.redirect("/404".into())),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Redirect> {
        if let Ok(address) = CashAddress::parse_cow(query.into()) {
            return Ok(self.redirect(format!("/address/{}", address.as_str())));
        }
        let bytes = from_be_hex(query)?;
        let unknown_hash = Sha256d::from_slice(&bytes)?;

        if self.chronik.tx(&unknown_hash).await.is_ok() {
            return Ok(self.redirect(format!("/tx/{}", query)));
        }
        if self.chronik.block_by_hash(&unknown_hash).await.is_ok() {
            return Ok(self.redirect(format!("/block/{}", query)));
        }

        Ok(self.redirect("/404".into()))
    }

    pub fn redirect(&self, url: String) -> Redirect {
        Redirect::permanent(&url)
    }
}

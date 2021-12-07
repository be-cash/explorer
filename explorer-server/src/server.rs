use anyhow::{Result, anyhow, bail};
use bitcoin_cash::{Address, Script};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use warp::{Reply, http::Uri};
use serde::Serialize;
use chrono::{Utc, TimeZone};
use chrono_humanize::HumanTime;
use std::{borrow::Cow, collections::{BTreeSet, HashMap, hash_map::Entry}, convert::{TryInto, TryFrom}, sync::Arc};
use zerocopy::{AsBytes, byteorder::{I32, U32}};

use crate::{blockchain::{BlockHeader, Destination, destination_from_script, is_coinbase, from_le_hex, to_legacy_address, to_le_hex}, formatting::{render_amount, render_byte_size, render_difficulty, render_integer, render_integer_smallify, render_sats}, grpc::bchrpc, indexdb::{AddressBalance, TxOutSpend}, indexer::Indexer, primitives::{SlpAction, TokenMeta, TxMeta, TxMetaVariant}};

pub struct Server {
    indexer: Arc<dyn Indexer>,
    satoshi_addr_prefix: &'static str,
    tokens_addr_prefix: &'static str,
}

impl Server {
    pub async fn setup(indexer: Arc<dyn Indexer>) -> Result<Self> {
        let satoshi_addr_prefix = "ecash";
        Ok(Server {
            indexer,
            satoshi_addr_prefix,
            tokens_addr_prefix: "etoken",
        })
    }
}

impl Server {
    pub async fn dashboard(&self) -> Result<impl Reply> {
        let markup = html! {
            (DOCTYPE)
            head {
                meta charset="utf-8";
                title { "be.cash Block Explorer" }
                (self.head_common())
            }
            body {
                (self.toolbar())

                .ui.container.homepage__welcome {
                    h1 {
                        "Welcome to the be.cash Block Explorer"
                    }
                    p {
                        "We welcome your feedback and bug reports to contact@be.cash."
                    }
                }

                .homepage__ludwig {
                    .homepage__ludwig-circle {}
                    img.homepage__ludwig-image src="/assets/ludwig.png" {}
                }

                .ocean {
                    .wave { }
                    .wave { }
                }
                
                (self.footer())
            }
        };
        Ok(warp::reply::html(markup.into_string()))
    }

    fn render_pagination(&self, page: usize, last_page: usize, curated_page_offsets: &[usize], query_str: &str) -> Markup {
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
        html! {
            .bottom-pagination {
                p {}
                .ui.pagination.menu {
                    @for &page in pages.iter() {
                        @if !pages.contains(&page.saturating_sub(1)) {
                            @if page.checked_sub(2).map(|page| pages.contains(&page)).unwrap_or(false) {
                                a.item href={(query_str) ((page - 2))} {
                                    ((page - 1))
                                }
                            } @else {
                                .item.disabled { "..." }
                            }
                        }
                        a.item href={(query_str) (page)} {
                            (page)
                        }
                    }
                }
            }
        }
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
        let markup = html! {
            (DOCTYPE)
            head {
                meta charset="utf-8";
                title { "be.cash Block Explorer" }
                (self.head_common())
            }
            body {
                (self.toolbar())

                .ui.container {
                    #blocks-table {}
                }

                (self.render_pagination(page as usize, last_page as usize, curated_page_offsets, "?page="))

                (self.footer())
                
                script type="text/javascript" src={"/data/blocks/" (first_page_begin) "/" (first_page_end) "/dat.js?v=0.2"} {}
                script type="text/javascript" src={"/data/blocks/" (second_page_begin) "/" (second_page_end) "/dat.js?v=0.2"} {}
                script type="text/javascript" src="/code/blocks.js" {}
            }
        };
        Ok(warp::reply::html(markup.into_string()))
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
        let encoded_blocks = serde_json::to_string(&json_blocks)?;
        let reply = format!(r#"
            if (window.blockData === undefined)
                window.blockData = [];
            {{
                var blocks = JSON.parse('{encoded_blocks}');
                var startIdx = window.blockData.length;
                window.blockData.length += blocks.length;
                for (var i = 0; i < blocks.length; ++i) {{
                    var block = blocks[i];
                    window.blockData[startIdx + i] = {{
                        hash: block.hash,
                        height: block.height,
                        version: block.version,
                        timestamp: new Date(block.timestamp * 1000),
                        difficulty: block.difficulty,
                        size: block.size,
                        medianTime: block.medianTime,
                        numTxs: block.numTxs,
                    }};
                }}
            }}
        "#, encoded_blocks = encoded_blocks);
        let reply = warp::reply::with_header(reply, "content-type", "application/javascript");
        let reply = warp::reply::with_header(reply, "last-modified", "Tue, 29 Dec 2020 06:31:27 GMT");
        Ok(reply)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTx {
    tx_hash: String,
    block_height: Option<i32>,
    timestamp: i64,
    is_coinbase: bool,
    size: i32,
    num_inputs: u32,
    num_outputs: u32,
    sats_input: i64,
    sats_output: i64,
    delta_sats: i64,
    delta_tokens: i64,
    token_idx: Option<usize>,
    is_burned_slp: bool,
    token_input: u64,
    token_output: u64,
    slp_action: Option<SlpAction>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonToken {
    token_id: String,
    token_type: u32,
    token_ticker: String,
    token_name: String,
    decimals: u32,
    group_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTxs {
    txs: Vec<JsonTx>,
    tokens: Vec<JsonToken>,
    token_indices: HashMap<Vec<u8>, usize>,
}

impl JsonToken {
    fn from_token_meta(token_id: &[u8], token_meta: TokenMeta) -> Self {
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
        
        let markup = html! {
            (DOCTYPE)
            head {
                title { "be.cash Block Explorer" }
                (self.head_common())
            }
            body {
                (self.toolbar())

                .ui.container {
                    h1 {
                        "Block #"
                        (block_meta.height)
                    }
                    .ui.segment {
                        strong { "Hash: " }
                        span.hex { (block_hash_str) }
                    }
                    .ui.grid {
                        .six.wide.column {
                            table.ui.table {
                                tbody {
                                    tr {
                                        td { "Age" }
                                        td { (HumanTime::from(timestamp)) }
                                    }
                                    tr {
                                        td { "Mined on" }
                                        td { (self.render_timestamp(block_meta.timestamp)) }
                                    }
                                    tr {
                                        td { "Unix Timestamp" }
                                        td { (render_integer(block_meta.timestamp as u64)) }
                                    }
                                    tr {
                                        td { "Mined by" }
                                        td { "Unknown" }
                                    }
                                    tr {
                                        td { "Confirmations" }
                                        td { (confirmations) }
                                    }
                                    tr {
                                        td { "Size" }
                                        td { (render_byte_size(block_meta.size, true)) }
                                    }
                                    tr {
                                        td { "Transactions" }
                                        td { (block_meta.num_txs) }
                                    }
                                }
                            }
                        }
                        .ten.wide.column {
                            table.ui.table {
                                tbody {
                                    tr {
                                        td { "Difficulty" }
                                        td { (render_difficulty(block_meta.difficulty)) }
                                    }
                                    tr {
                                        td { "Header" }
                                        td {
                                            .hex {
                                                (hex::encode(block_header.as_bytes()))
                                            }
                                        }
                                    }
                                    tr {
                                        td { "Nonce" }
                                        td { (block_meta.nonce) }
                                    }
                                    tr {
                                        td { "Coinbase data" }
                                        td {
                                            (String::from_utf8_lossy(&block_meta.coinbase_data))
                                        }
                                    }
                                    tr {
                                        td { "Coinbase hex" }
                                        td {
                                            .hex {
                                                (hex::encode(&block_meta.coinbase_data))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    .ui.segment {
                        h2.ui.header { "Transactions" }
                        #block-txs {
                            #txs-table {}
                        }
                    }
                }

                (self.footer())

                script type="text/javascript" src={"/data/block/" (block_hash_str) "/dat.js"} {}
                script type="text/javascript" src="/code/txs.js" {}
            }
        };
        Ok(warp::reply::html(markup.into_string()))
    }

    pub async fn tx(&self, tx_hash_str: &str) -> Result<impl Reply> {
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
        let block_meta = self.indexer.db().block_meta(&tx.transaction.block_hash)?;
        let best_height = self.indexer.db().last_block_height()?;
        let confirmations = match &block_meta {
            Some(block_meta) => best_height - block_meta.height as u32 + 1,
            None => 0,
        };
        let timestamp = Utc.timestamp(tx.transaction.timestamp, 0);
        let markup = html! {
            (DOCTYPE)
            head {
                title { "be.cash Block Explorer" }
                (self.head_common())
            }
            body {
                (self.toolbar())

                .ui.container {
                    .ui.grid {
                        .ten.wide.column {
                            h1.tx-header__title { (title) }

                            @if tx.tx_meta.is_coinbase {
                                .tx-header__label.ui.green.label { "Coinbase" }
                            }
                        }
                        .six.wide.column {
                            .tx-transaction__toggle-wrapper {
                                .ui.slider.checkbox.tx-transaction__toggle {
                                    input
                                        type="checkbox"
                                        onclick="$('#raw-hex').toggle()";
                                    label { "Show raw hex" }
                                }
                            }
                        }
                    }
                    #tx-hash.ui.segment.tx-details {
                        table.tx-hash-table.ui.very.basic.table {
                            tbody {
                                tr {
                                    td.no-padding[token_hash_str.is_none()] { strong { "Transaction ID" } }
                                    td.no-padding[token_hash_str.is_none()] { span.hex { (tx_hash_str) } }
                                }
                                @if let Some(hash) = token_hash_str {
                                    tr {
                                        td { strong { "Token ID" } }
                                        td {
                                            span.hex { (hash) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    #raw-hex.ui.segment style="display: none;" {
                        h4 { "Raw Transaction Hex" }
                        .hex {
                            (hex::encode(&tx.raw_tx))
                        }
                    }

                    h2 { "Details" }
                    .ui.grid.segment.tx-details {
                        table.tx-details-table.ui.very.basic.table {
                            tbody {
                                tr {
                                    td { "Age" }
                                    td { (HumanTime::from(timestamp)) }
                                }
                                tr {
                                    td { "Block" }
                                    td {
                                        @match &block_meta {
                                            Some(_) => {
                                                a href={"/block/" (to_le_hex(&tx.transaction.block_hash))} {
                                                    (render_integer_smallify(tx.transaction.block_height as u64))
                                                }
                                                " ("
                                                (render_integer_smallify(confirmations as u64))
                                                " confirmations)"
                                            },
                                            None => "Not mined yet",
                                        }
                                    }
                                }
                                tr {
                                    td { "Unix Timestamp" }
                                    td {
                                        @match &block_meta {
                                            Some(block_meta) => (render_integer(block_meta.timestamp as u64)),
                                            None => "Not mined yet",
                                        }
                                    }
                                }
                                tr {
                                    td { "Size" }
                                    td { (render_byte_size(tx.transaction.size as u64, true)) }
                                }
                                tr {
                                    td { "Locktime" }
                                    td { (render_integer_smallify(tx.transaction.lock_time as u64)) }
                                }
                            }
                        }
                        .ui.vertical.divider.tx-details-table__divider {}
                        table.tx-details-table.ui.very.basic.table {
                            tbody {
                                tr {
                                    td { "Mined on" }
                                    td {
                                        @match &block_meta {
                                            Some(block_meta) => (self.render_timestamp(block_meta.timestamp)),
                                            None => "Not mined yet",
                                        }
                                    }
                                }
                                tr {
                                    td { "Total Input" }
                                    td { (render_sats(tx.tx_meta.sats_input)) " XEC" }
                                }
                                tr {
                                    td { "Total Output" }
                                    td { (render_sats(tx.tx_meta.sats_output)) " XEC" }
                                }
                                tr {
                                    td { "Fee" }
                                    td { (render_sats((tx.tx_meta.sats_input - tx.tx_meta.sats_output).max(0))) " XEC" }
                                }
                                tr {
                                    td { "Version" }
                                    td { (tx.transaction.version) }
                                }
                            }
                        }
                    }

                    @match self.render_token_info(&tx.tx_meta.variant, &tx.token_meta) {
                        Some(token_info_markup) => {
                            (self.render_token_info_title(&tx.tx_meta.variant, &tx.token_meta))
                            .ui.grid.segment.tx-details {
                                (token_info_markup)
                            }
                        },
                        None => {},
                    }

                    .ui.grid {
                        .ten.wide.column {
                            h2 { "Transaction" }
                        }
                        .six.wide.column {
                            .tx-transaction__toggle-wrapper {
                                .ui.slider.checkbox.tx-transaction__toggle {
                                    input
                                        type="checkbox"
                                        onclick="toggleTransactionScriptData()";
                                    label { "Show all scripts" }
                                }
                            }
                        }
                    }
                    .ui.grid.segment {
                        .seven.wide.column {
                            h4 { "Inputs (" (&tx.transaction.inputs.len()) ")" }

                            (PreEscaped(
                                r#"<script type="text/javascript">
                                    var detailsOpen = {};
                                    function toggleDetails(kind, idx) {{
                                        var key = kind + idx
                                        if (detailsOpen[key]) {{
                                            $('#' + kind + '-details-' + idx).hide();
                                            $('#' + kind + '-details-toggle-' + idx).removeClass('up').addClass('down');
                                        }} else {{
                                            $('#' + kind + '-details-' + idx).show();
                                            $('#' + kind + '-details-toggle-' + idx).removeClass('down').addClass('up');
                                        }}
                                        detailsOpen[key] = !detailsOpen[key];
                                    }}
                                </script>"#,
                            ))
                            table#inputs.ui.very.basic.table {
                                tbody {
                                    @for input in &tx.transaction.inputs {
                                        (self.render_input(input, &tx.token_meta))
                                    }
                                }
                            }
                        }
                        .two.wide.column {
                            .tx-transaction__arrow-separator {
                                i.big.icon.arrow.right {}
                            }
                        }
                        .seven.wide.column {
                            h4 { "Outputs (" (&tx.transaction.outputs.len()) ")" }

                            table#outputs.ui.very.basic.table {
                                tbody {
                                    @for output in &tx.transaction.outputs {
                                        (self.render_output(output, &tx.token_meta, &tx.tx_out_spends))
                                    }
                                }
                            }
                        }
                    }
                }

                (self.footer())
            }
        };
        Ok(warp::reply::html(markup.into_string()))
    }

    fn render_token_info_title(&self, variant: &TxMetaVariant, token_meta: &Option<TokenMeta>) -> Markup {
        use SlpAction::*;
        match (variant, token_meta) {
            (
                TxMetaVariant::Slp { action, .. },
                Some(_),
            ) => html! {
                @let action_str = match action {
                    SlpV1Genesis => "GENESIS",
                    SlpV1Mint => "MINT",
                    SlpV1Send => "SEND",
                    SlpV1Nft1GroupGenesis => "NFT1 Group GENESIS",
                    SlpV1Nft1GroupMint => "NFT1 MINT",
                    SlpV1Nft1GroupSend => "NFT1 Group SEND",
                    SlpV1Nft1UniqueChildGenesis => "NFT1 Child GENESIS",
                    SlpV1Nft1UniqueChildSend => "NFT1 Child SEND",
                };
                h2 {
                    "Token Details (" (action_str) " Transaction)"
                }
            },
            (
                TxMetaVariant::InvalidSlp { .. },
                Some(_)
            ) => html! {
                h2 {
                    "Token Details (Invalid Transaction)"
                }
            },
            (
                TxMetaVariant::InvalidSlp { .. },
                None
            ) => html! {
                h2 {
                    "Token Details (Invalid Transaction; Unknown Token)"
                }
            },
            _ => html! {},
        }
    }

    fn render_token_info(&self, variant: &TxMetaVariant, token_meta: &Option<TokenMeta>) -> Option<Markup> {
        use SlpAction::*;
        match (variant, token_meta) {
            (
                TxMetaVariant::Slp { token_id, action, token_input, token_output },
                Some(token_meta),
            ) => Some(html! {
                @let ticker = String::from_utf8_lossy(&token_meta.token_ticker);
                @let action_str = match action {
                    SlpV1Genesis => "GENESIS",
                    SlpV1Mint => "MINT",
                    SlpV1Send => "SEND",
                    SlpV1Nft1GroupGenesis => "NFT1 Group GENESIS",
                    SlpV1Nft1GroupMint => "NFT1 MINT",
                    SlpV1Nft1GroupSend => "NFT1 Group SEND",
                    SlpV1Nft1UniqueChildGenesis => "NFT1 Child GENESIS",
                    SlpV1Nft1UniqueChildSend => "NFT1 Child SEND",
                };
                table.tx-details-table.ui.very.basic.table {
                    tbody {
                        tr {
                            td { "Token Ticker" }
                            td { (ticker) }
                        }
                        tr {
                            td { "Token Name" }
                            td {
                                (String::from_utf8_lossy(&token_meta.token_name))
                                @if action_str != "GENESIS" {
                                    " ("
                                    a href={"/tx/" (hex::encode(&token_id))} { "GENESIS" }
                                    ")"
                                }
                            }
                        }
                        tr {
                            td { "Token Type" }
                            td {
                                @match token_meta.token_type {
                                    0x01 => {
                                        "Type1 ("
                                        a href="https://github.com/simpleledger/slp-specifications/blob/master/slp-token-type-1.md" {
                                            "Specification"
                                        }
                                        ")"
                                    }
                                    0x41 => {
                                        "NFT1 Child ("
                                        a href="https://github.com/simpleledger/slp-specifications/blob/master/slp-nft-1.md" {
                                            "Specification"
                                        }
                                        ")"
                                    }
                                    0x81 => {
                                        "NFT1 Group ("
                                        a href="https://github.com/simpleledger/slp-specifications/blob/master/slp-nft-1.md" {
                                            "Specification"
                                        }
                                        ")"
                                    }
                                    token_type => { "Unknown type: " (token_type) }
                                }
                            }
                        }
                        tr {
                            td { "Transaction Type" }
                            td { (action_str) }
                        }
                    }
                }
                .ui.vertical.divider.tx-details-table__divider {}
                table.tx-details-table.ui.very.basic.table {
                    tbody {
                        tr {
                            td { "Token Output" }
                            td {
                                (render_amount(*token_output, token_meta.decimals)) " " (ticker)
                                @if token_output < token_input {
                                    br;
                                    " ("
                                    (render_amount(token_input - token_output, token_meta.decimals))
                                    " " (ticker) " burned)"
                                }
                            }
                        }
                        tr {
                            td { "Document URI" }
                            td {
                                @let token_url = String::from_utf8_lossy(&token_meta.token_document_url);
                                a href={(token_url)} target="_blank" { (token_url) }
                            }
                        }
                        tr {
                            td { "Document Hash" }
                            td {
                                @match token_meta.token_document_url.len() {
                                    0 => .ui.black.horizontal.label { "Not set" },
                                    _ => .hex { (hex::encode(&token_meta.token_document_hash)) },
                                }
                            }
                        }
                        tr {
                            td { "Decimals" }
                            td { (token_meta.decimals) }
                        }
                    }
                }
            }),
            (
                TxMetaVariant::InvalidSlp { token_input, .. },
                Some(token_meta)
            ) => Some(html! {
                @let ticker = String::from_utf8_lossy(&token_meta.token_ticker);
                table.ui.very.basic.table {
                    tbody {
                        tr {
                            td { "Token Ticker" }
                            td { (ticker) }
                        }
                        tr {
                            td { "Token Name" }
                            td { (String::from_utf8_lossy(&token_meta.token_name)) }
                        }
                        tr {
                            td { "Tokens burned" }
                            td {
                                (render_amount(*token_input, token_meta.decimals)) " " (ticker)
                            }
                        }
                    }
                }
            }),
            (
                TxMetaVariant::InvalidSlp { token_input, .. },
                None
            ) => Some(html! {
                table.ui.very.basic.table {
                    tbody {
                        tr {
                            td { "Token Ticker" }
                            td { "Unknown" }
                        }
                        tr {
                            td { "Tokens burned" }
                            td {
                                (render_integer_smallify(*token_input))
                            }
                        }
                    }
                }
            }),
            _ => None,
        }
    }

    pub fn render_output(
        &self,
        tx_output: &bchrpc::transaction::Output,
        token_meta: &Option<TokenMeta>,
        tx_out_spends: &HashMap<u32, Option<TxOutSpend>>,
    ) -> Markup {
        let is_token = tx_output.slp_token.as_ref().map(|slp| slp.amount > 0 || slp.is_mint_baton).unwrap_or(false);
        let destination = destination_from_script(
            if is_token { self.tokens_addr_prefix } else { self.satoshi_addr_prefix },
            &tx_output.pubkey_script,
        );
        let output_script = Script::deser_ops(tx_output.pubkey_script.as_slice().into())
            .map(|script| script.to_string())
            .unwrap_or("invalid script".to_string());
        html! {
            tr {
                td {
                    @match tx_out_spends.get(&tx_output.index) {
                        Some(Some(tx_out_spend)) => {
                            a href={"/tx/" (to_le_hex(&tx_out_spend.by_tx_hash))} {
                                i.icon.sign.out {}
                            }
                        }
                        Some(None) => {
                        }
                        None => {
                            @if let Destination::Nulldata(_) = &destination {
                                i.icon.sticky.note.outline {}
                            } @else {
                                i.icon.question {}
                            }
                        }
                    }
                }
                td { (tx_output.index) }
                td {
                    @if is_token {
                        img src="/assets/slp-logo.png" {}
                    }
                }
                td {
                    .destination.hex {
                        @match &destination {
                            Destination::Address(address) => {a href={"/address/" (address.cash_addr())} {
                                (address.cash_addr())
                            }},
                            Destination::Nulldata(_ops) => "OP_RETURN data",
                            Destination::P2PK(pubkey) => {"Pubkey: " (hex::encode(&pubkey))},
                            Destination::Unknown(_bytes) => "Unknown",
                        }
                    }
                }
                td {
                    .amount.hex {
                        @match (&tx_output.slp_token, token_meta) {
                            (Some(slp), Some(token)) if slp.amount > 0 || slp.is_mint_baton => {
                                @if slp.is_mint_baton {
                                    .ui.green.horizontal.label { "Mint baton" }
                                } @else {
                                    (render_amount(slp.amount, slp.decimals))
                                    " "
                                    (String::from_utf8_lossy(&token.token_ticker))
                                }
                                div {
                                    small {
                                        (render_sats(tx_output.value))
                                        " XEC"
                                    }
                                }
                            }
                            (Some(slp), Some(_)) if slp.is_mint_baton => {
                                .ui.green.horizontal.label { "Mint baton" }
                            }
                            _ => {
                                (render_sats(tx_output.value))
                                " XEC"
                            }
                        }
                    }
                }
            }
            tr.tx-transaction__script-data.hidden {
                td colspan="1" {}
                td colspan="5" {
                    p {
                        strong { "Script Hex" }
                        .hex { (hex::encode(&tx_output.pubkey_script)) }
                    }
                    p {
                        strong { "Script Decoded" }
                        .hex { (output_script) }
                    }
                }
            }
        }
    }

    pub fn render_input(
        &self,
        tx_input: &bchrpc::transaction::Input,
        token_meta: &Option<TokenMeta>,
    ) -> Markup {
        let is_token = tx_input.slp_token.as_ref().map(|slp| slp.amount > 0 || slp.is_mint_baton).unwrap_or(false);
        let outpoint = tx_input.outpoint.as_ref().expect("No outpoint");
        let destination = destination_from_script(
            if is_token { self.tokens_addr_prefix } else { self.satoshi_addr_prefix },
            &tx_input.previous_script,
        );
        let input_script = Script::deser_ops(tx_input.signature_script.as_slice().into())
            .map(|script| script.to_string())
            .unwrap_or("invalid script".to_string());
        html! {
            tr {
                @if is_coinbase(outpoint) {
                    td {
                        .ui.green.horizontal.label { "Coinbase" }
                    }
                } @else {
                    td {
                        a href={"/tx/" (to_le_hex(&outpoint.hash))} {
                            i.horizontally.flipped.icon.sign.out {}
                        }
                    }
                    td {
                        (tx_input.index)
                    }
                    td {
                        @if is_token {
                            img src="/assets/slp-logo.png" {}
                        }
                    }
                    td {
                        .destination.hex {
                            @match &destination {
                                Destination::Address(address) => {a href={"/address/" (address.cash_addr())} {
                                    (address.cash_addr())
                                }},
                                Destination::P2PK(pubkey) => {"Pubkey: " (hex::encode(&pubkey))},
                                Destination::Unknown(_bytes) => "Unknown",
                                Destination::Nulldata(_ops) => "Unreachable",
                            }
                        }
                    }
                }
                td {
                    .amount.hex {
                        @match (&tx_input.slp_token, token_meta) {
                            (Some(slp), Some(token)) if slp.amount > 0 || slp.is_mint_baton => {
                                @if slp.is_mint_baton {
                                    .ui.green.horizontal.label { "Mint baton" }
                                } @else {
                                    (render_amount(slp.amount, slp.decimals))
                                    " "
                                    (String::from_utf8_lossy(&token.token_ticker))
                                }
                                div {
                                    small {
                                        (render_sats(tx_input.value))
                                        " XEC"
                                    }
                                }
                            }
                            _ => {
                                (render_sats(tx_input.value))
                                " XEC"
                            }
                        }
                    }
                }
            }
            tr.tx-transaction__script-data.hidden {
                td colspan="1" {}
                td colspan="5" {
                    p {
                        strong { "Script Hex" }
                        .hex { (hex::encode(&tx_input.signature_script)) }
                    }
                    p {
                        strong { "Script Decoded" }
                        .hex { (input_script) }
                    }
                }
            }
        }
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
        let last_tx_page = address_num_txs / page_size;
        let curated_page_offsets = &[
            1, 2, 3, 4, 5, 10, 20, 100, 1000, 2000,
        ];
        let mut json_txs = self.json_txs(
            address_txs
                .iter()
                .map(|(tx_hash, addr_tx, tx_meta)| {
                    (tx_hash.as_ref(), addr_tx.timestamp, Some(addr_tx.block_height), tx_meta, (addr_tx.delta_sats, addr_tx.delta_tokens))
                })
        ).await?;
        let balance = self.indexer.db().address_balance(&sats_address, coins_page * page_size, page_size)?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct JsonUtxo {
            tx_hash: String,
            out_idx: u32,
            sats_amount: i64,
            token_amount: u64,
            is_coinbase: bool,
            block_height: i32,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct JsonBalance {
            token_idx: Option<usize>,
            sats_amount: i64,
            token_amount: u64,
            utxos: Vec<JsonUtxo>,
        }
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
        let json_balances = json_balances.into_iter().map(|(_, balance)| balance).collect::<Vec<_>>();

        let encoded_txs = serde_json::to_string(&json_txs.txs)?.replace("'", r"\'");
        let encoded_tokens = serde_json::to_string(&json_txs.tokens)?.replace("'", r"\'");
        let encoded_balances = serde_json::to_string(&json_balances)?.replace("'", r"\'");
        let markup = html! {
            (DOCTYPE)
            head {
                title { "be.cash Block Explorer" }
                (self.head_common())
            }
            body {
                (self.toolbar())

                (PreEscaped(format!(
                    r#"<script type="text/javascript">
                        window.addrTxData = [];
                        window.addrBalances = [];
                        {{
                            var txs = JSON.parse('{encoded_txs}');
                            var tokens = JSON.parse('{encoded_tokens}');
                            var startIdx = window.addrTxData.length;
                            window.addrTxData.length += txs.length;
                            for (var i = 0; i < txs.length; ++i) {{
                                var tx = txs[i];
                                tx.token = tx.tokenIdx === null ? null : tokens[tx.tokenIdx];
                                tx.timestamp *= 1000;
                                window.addrTxData[startIdx + i] = tx;
                            }}
                            var balances = JSON.parse('{encoded_balances}');
                            window.addrBalances.length = balances.length;
                            for (var i = 0; i < balances.length; ++i) {{
                                var balance = balances[i];
                                balance.token = balance.tokenIdx === null ? null : tokens[balance.tokenIdx];
                                window.addrBalances[i] = balance;
                            }}
                        }}
                    </script>"#,
                    encoded_txs = encoded_txs,
                    encoded_tokens = encoded_tokens,
                    encoded_balances = encoded_balances,
                )))

                .ui.container {
                    table#coins.ui.table {
                        @for (balance_idx, balance) in json_balances.iter().enumerate() {
                            @let token = balance.token_idx.and_then(|token_idx| json_txs.tokens.get(token_idx));
                            @match token {
                                None => {
                                    tr {
                                        td colspan="20" {
                                            .address-sats {
                                                .balance {
                                                    h4 { "Balance" }
                                                    h1 {
                                                        (render_sats(balance.sats_amount)) " XEC"
                                                        a.show-coins onclick="$('#sats-coins').toggle(); loadSatsTable();" {
                                                            "Show Coins " i.icon.chevron.circle.down {}
                                                        }
                                                    }
                                                    @if token_dust > 0 {
                                                        h3 {
                                                            "+" (render_sats(token_dust)) " XEC in token dust"
                                                        }
                                                    }
                                                    @match address_num_txs {
                                                        1 => (address_num_txs) " Transaction",
                                                        _ => (address_num_txs) " Transactions",
                                                    }
                                                    table.addresses.ui.table.very.basic.collapsing.celled.compact {
                                                        tbody {
                                                            tr {
                                                                td { "Cash Address" }
                                                                td { (sats_address.cash_addr()) }
                                                            }
                                                            tr {
                                                                td { "Token Address" }
                                                                td { (token_address.cash_addr()) }
                                                            }
                                                            tr {
                                                                td { "Legacy Address" }
                                                                td { (legacy_address) }
                                                            }
                                                        }
                                                    }
                                                }
                                                .qr-code {
                                                    img#qr-code-img src={"/address-qr/" (address.cash_addr())} {}
                                                }
                                                .qr-kind id={"selected-address-" (if sats_address.cash_addr() == address.cash_addr() { "1" } else { "2"})} {
                                                    .address1 {
                                                        a onclick={"\
                                                            $('#qr-code-img').attr('src', '/address-qr/" (sats_address.cash_addr()) "');\
                                                            $('.qr-kind').attr('id', 'selected-address-1');\
                                                        "} {
                                                            "XEC Address"
                                                        }
                                                    }
                                                    .address2 {
                                                        a onclick={"\
                                                            $('#qr-code-img').attr('src', '/address-qr/" (token_address.cash_addr()) "');\
                                                            $('.qr-kind').attr('id', 'selected-address-2');\
                                                        "} {
                                                            "eToken Address"
                                                        }
                                                    }
                                                    .address3 {
                                                        a onclick={"\
                                                            $('#qr-code-img').attr('src', '/address-qr/" (legacy_address) "');\
                                                            $('.qr-kind').attr('id', 'selected-address-3');\
                                                        "} {
                                                            "Legacy Address"
                                                        }
                                                    }
                                                }
                                            }
                                            #sats-coins style="display: none;" {
                                                #sats-coins-table {}
                                            }
                                            script type="text/javascript" src="/code/coins.js?v=1" {}
                                        }
                                    }
                                },
                                Some(token) => {
                                    tr {
                                        td.token-amount {
                                            (render_amount(balance.token_amount, token.decimals))
                                        }
                                        td {
                                            (PreEscaped(&token.token_ticker))
                                        }
                                        td {
                                            (PreEscaped(&token.token_name))
                                        }
                                        td {
                                            "+" (render_sats(balance.sats_amount))
                                            " XEC dust"
                                            a onclick={"$('#token-coins-" (balance_idx) "').toggle(); loadTokenTable(" (balance_idx) ")"} {
                                                " ("
                                                (render_integer_smallify(balance.utxos.len() as u64))
                                                " coins "
                                                i.icon.chevron.circle.down {}
                                                ")"
                                            }
                                        }
                                    }
                                    tr id={"token-coins-" (balance_idx)} style="display: none;" {
                                        td.token-table colspan="20" {
                                            div id={"tokens-coins-table-" (balance_idx)} {}
                                        }
                                    }
                                },
                            }
                        }
                    }
                    #addr-txs {
                        #txs-table {}
                    }
                }
                @if last_tx_page != 0 {
                    (self.render_pagination(txs_page, last_tx_page, curated_page_offsets, "?tx_page="))
                }

                (self.footer())
            }
        };
        Ok(warp::reply::html(markup.into_string()))
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

impl Server {
    fn head_common(&self) -> Markup {
        html! {
            meta charset="utf-8";
            script
                src="https://code.jquery.com/jquery-3.1.1.min.js"
                integrity="sha256-hVVnYaiADRTO2PzUGmuLJr8BLUSjGIZsDYGmIJLv2b8="
                crossorigin="anonymous" {}
            script type="text/javascript" src="/code/semantic-ui/semantic.min.js?v=0" {}
            script type="text/javascript" src="/code/webix/webix.min.js?v=8.1.0" {}
            script type="text/javascript" src="/code/moment.min.js?v=0" {}
            script type="text/javascript" src="/code/common.js" {}
            link rel="stylesheet" href="/code/webix/webix.min.css";
            link rel="stylesheet" href="/code/semantic-ui/semantic.min.css";
            link rel="stylesheet" href="/code/styles/index.css";
            link rel="preconnect" href="https://fonts.gstatic.com";
            link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Ubuntu+Mono&display=swap";
        }
    }

    fn toolbar(&self) -> Markup {
        html! {
            .ui.main.menu {
                a.header.item href="/" {
                    img.logo src="/assets/logo.png" {}
                }
                .item {
                    #search-box.ui.transparent.icon.input {
                        input#search-bar
                            type="text"
                            placeholder="Search blocks, transactions, adddresses, tokens..."
                            onchange="searchBarChange()"
                            onkeyup="searchBarChange()" {}
                        i#search-button.search.link.icon
                            onclick="searchButton()" {}
                    }
                }
                a.right.floated.item href="/blocks" { "Blocks" }
            }
            // script { (PreEscaped(r#"
            //     $('.main.menu  .ui.dropdown').dropdown({
            //         on: 'hover'
            //     });
            // "#)) }
        }
    }

    fn footer(&self) -> Markup {
        html! {
            #footer.ui.inverted.vertical.footer.segment {
                .ui.container {
                    "be.cash Explorer"
                }
            }
        }
    }

    fn render_timestamp(&self, timestamp: i64) -> Markup {
        html! {
            (PreEscaped(format!(
                r#"<script type="text/javascript">
                    document.write(moment({timestamp}).format('L LTS'));
                    document.write(' <small>(UTC' + tzOffset + ')</small>');
                </script>"#,
                timestamp=timestamp * 1000,
            )))
        }
    }
}

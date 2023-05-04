use std::collections::HashMap;

use bitcoinsuite_chronik_client::proto;
use bitcoinsuite_core::{CashAddress, Hashed, Sha256d};
use bitcoinsuite_error::Result;

use crate::{
    blockchain::to_be_hex,
    server_primitives::{
        JsonSlpv2Section, JsonSlpv2SectionStats, JsonSlpv2TokenInfo, JsonTx, JsonTxStats,
    },
};

pub fn tokens_to_json(
    tokens: &HashMap<String, proto::Slpv2TokenInfo>,
) -> Result<HashMap<String, JsonSlpv2TokenInfo>> {
    let mut json_tokens = HashMap::new();

    for (token_id, token) in tokens.iter() {
        if let Some(genesis_info) = &token.genesis_data {
            let token_ticker = String::from_utf8_lossy(&genesis_info.token_ticker).to_string();
            let token_name = String::from_utf8_lossy(&genesis_info.token_name).to_string();
            let token_url = String::from_utf8_lossy(&genesis_info.url).to_string();

            let json_token = JsonSlpv2TokenInfo {
                token_id: token_id.clone(),
                token_type: token.token_type as u32,
                token_ticker,
                token_name,
                token_url,
                decimals: genesis_info.decimals,
                token_color: crate::templating::filters::to_token_color(&token.token_id).unwrap(),
            };
            json_tokens.insert(token_id.clone(), json_token.clone());
        }
    }

    Ok(json_tokens)
}

pub fn tx_history_to_json(
    address: &CashAddress,
    address_tx_history: proto::TxHistoryPage,
    json_tokens: &HashMap<String, JsonSlpv2TokenInfo>,
) -> Result<Vec<JsonTx>> {
    let mut json_txs = Vec::new();
    let address_bytes = address.to_script().bytecode().to_vec();

    for tx in address_tx_history.txs.iter() {
        let (block_height, timestamp) = match &tx.block {
            Some(block) => (Some(block.height), block.timestamp),
            None => (None, tx.time_first_seen),
        };

        let mut slpv2_sections = Vec::new();
        for section in &tx.slpv2_sections {
            let token_id = Sha256d::from_slice(&section.token_id)?;
            if let Some(token_info) = json_tokens.get(&token_id.to_string()) {
                slpv2_sections.push(JsonSlpv2Section {
                    token_info: token_info.clone(),
                    stats: calc_section_stats(tx, section, Some(&address_bytes)),
                });
            }
        }

        let stats = calc_tx_stats(tx, Some(&address_bytes));

        json_txs.push(JsonTx {
            tx_hash: to_be_hex(&tx.txid),
            block_height,
            timestamp,
            is_coinbase: tx.is_coinbase,
            size: tx.size as i32,
            num_inputs: tx.inputs.len() as u32,
            num_outputs: tx.outputs.len() as u32,
            stats,
            slpv2_sections,
        });
    }

    Ok(json_txs)
}

pub fn block_txs_to_json(
    block: proto::Block,
    block_txs: &[proto::Tx],
    tokens_by_hex: &HashMap<String, proto::Slpv2TokenInfo>,
) -> Result<Vec<JsonTx>> {
    let mut json_txs = Vec::new();

    for tx in block_txs.iter() {
        let (block_height, timestamp) = match &block.block_info {
            Some(block_info) => (Some(block_info.height), block_info.timestamp),
            None => (None, 0),
        };

        let mut slpv2_sections = Vec::new();
        for section in &tx.slpv2_sections {
            let token_id = Sha256d::from_slice(&section.token_id)?;
            let token_info = tokens_by_hex
                .get(&token_id.to_string())
                .and_then(|token_info| token_info.genesis_data.as_ref());
            let default_genesis_data = proto::Slpv2GenesisData::default();
            let genesis_data = token_info.unwrap_or(&default_genesis_data);

            let token_ticker = String::from_utf8_lossy(&genesis_data.token_ticker).to_string();
            let token_name = String::from_utf8_lossy(&genesis_data.token_name).to_string();
            let token_url = String::from_utf8_lossy(&genesis_data.url).to_string();
            slpv2_sections.push(JsonSlpv2Section {
                token_info: JsonSlpv2TokenInfo {
                    token_id: token_id.to_string(),
                    token_type: section.token_type as u32,
                    token_ticker,
                    token_name,
                    token_url,
                    decimals: genesis_data.decimals,
                    token_color: crate::templating::filters::to_token_color(token_id.as_slice()).unwrap(),
                },
                stats: calc_section_stats(tx, section, None),
            });
        }

        let stats = calc_tx_stats(tx, None);

        json_txs.push(JsonTx {
            tx_hash: to_be_hex(&tx.txid),
            block_height,
            timestamp,
            is_coinbase: tx.is_coinbase,
            size: tx.size as i32,
            num_inputs: tx.inputs.len() as u32,
            num_outputs: tx.outputs.len() as u32,
            stats,
            slpv2_sections,
        });
    }

    Ok(json_txs)
}

pub fn calc_tx_stats(tx: &proto::Tx, address_bytes: Option<&[u8]>) -> JsonTxStats {
    let sats_input = tx.inputs.iter().map(|input| input.value).sum();
    let sats_output = tx.outputs.iter().map(|output| output.value).sum();

    let mut delta_sats: i64 = 0;

    for input in &tx.inputs {
        if let Some(address_bytes) = address_bytes {
            if address_bytes != input.output_script {
                continue;
            }
        }
        delta_sats -= input.value;
    }

    for output in &tx.outputs {
        if let Some(address_bytes) = address_bytes {
            if address_bytes != output.output_script {
                continue;
            }
        }
        delta_sats += output.value;
    }

    JsonTxStats {
        sats_input,
        sats_output,
        delta_sats,
    }
}

pub fn calc_section_stats(
    tx: &proto::Tx,
    section: &proto::Slpv2Section,
    address_bytes: Option<&[u8]>,
) -> JsonSlpv2SectionStats {
    let token_input = tx
        .inputs
        .iter()
        .filter_map(|input| input.slpv2.as_ref())
        .filter(|token| token.token_id == section.token_id)
        .map(|token| token.amount)
        .sum::<i64>();
    let token_output = tx
        .outputs
        .iter()
        .filter_map(|output| output.slpv2.as_ref())
        .filter(|token| token.token_id == section.token_id)
        .map(|token| token.amount)
        .sum::<i64>();
    let does_burn_tokens =
        section.intentional_burn_amount > 0 || !tx.slpv2_burn_token_ids.is_empty();

    let mut delta_tokens: i64 = 0;

    for input in &tx.inputs {
        if let Some(address_bytes) = address_bytes {
            if address_bytes != input.output_script {
                continue;
            }
        }
        if let Some(slp) = &input.slpv2 {
            if slp.token_id == section.token_id {
                delta_tokens -= slp.amount;
            }
        }
    }

    for output in &tx.outputs {
        if let Some(address_bytes) = address_bytes {
            if address_bytes != output.output_script {
                continue;
            }
        }
        if let Some(slp) = &output.slpv2 {
            if slp.token_id == section.token_id {
                delta_tokens += slp.amount;
            }
        }
    }

    JsonSlpv2SectionStats {
        delta_tokens,
        token_input,
        token_output,
        does_burn_tokens,
    }
}

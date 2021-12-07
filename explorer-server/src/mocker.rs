use std::convert::TryInto;

use bitcoin_cash::{Opcode};

use crate::{grpc::bchrpc};
use bchrpc::{
    Block,
    BlockInfo,
    Transaction,
    SlpTransactionInfo,
    SlpV1GenesisMetadata,
    SlpToken,
    transaction::{Input, Output, input::Outpoint},
    slp_transaction_info::{TxMetadata},
    block::{TransactionData, transaction_data::{TxidsOrTxs}}
};
use anyhow::{Result};
use rand::{distributions::Alphanumeric, seq::SliceRandom, Rng}; // 0.8
use sha2::{Sha256, Digest};

const SLP_NAMES: [[&'static str; 2]; 8] = [
    ["HONK HONK", "HONK"],
    ["MIST", "MIST"],
    ["Mint", "MINT"],
    ["Spice", "SPICE"],
    ["MAZE", "MAZE"],
    ["HonestCoin", "USDH"],
    ["Tether USDt", "USDt"],
    ["flexUSD", "flexUSD"],
];

pub fn generate_random_sha256() -> Result<Vec<u8>> {
    let s: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(s);
    let result = hasher.finalize().as_slice().try_into()?;
    Ok(result)
}

pub fn generate_script() -> Result<Vec<u8>> {
    const OP_DUP: u8 = Opcode::OP_DUP as u8;
    const OP_HASH160: u8 = Opcode::OP_HASH160 as u8;
    const OP_EQUALVERIFY: u8 = Opcode::OP_EQUALVERIFY as u8;
    const OP_CHECKSIG: u8 = Opcode::OP_CHECKSIG as u8;

    let random_number: u8 = rand::thread_rng().gen_range(0..255);
    let mut script: Vec<u8> = vec![];

    let beginning: Vec<u8> = vec![OP_DUP, OP_HASH160, 20];
    let end: Vec<u8> = vec![OP_EQUALVERIFY, OP_CHECKSIG];

    script.extend(beginning);
    script.extend([random_number; 20]);
    script.extend(end);

    return Ok(script)
}

pub fn pick_random_slp_name_ticket_pair<'a>() -> Vec<&'a str> {
    SLP_NAMES.choose(&mut rand::thread_rng()).unwrap().to_vec()
}

pub fn generate_transaction(height: i32, block_hash: &Vec<u8>) -> Result<Transaction> {
    let slp_pair = pick_random_slp_name_ticket_pair();

    let tx_metadata = TxMetadata::V1Genesis(
        SlpV1GenesisMetadata{
            name: String::from(slp_pair[0]).into_bytes(),
            ticker: String::from(slp_pair[1]).into_bytes(),
            document_url: vec![],
            document_hash: vec![],
            decimals: 2,
            mint_baton_vout: 0,
            mint_amount: 123456
        }
    );

    let token_id = generate_random_sha256()?;

    let slp_token = SlpToken{
        token_id: token_id.to_vec(),
        amount: 12,
        is_mint_baton: false,
        address: String::from(""),
        decimals: 8,
        slp_action: 6,
        token_type: 1,
    };

    let slp_transaction_info = SlpTransactionInfo{
        slp_action: 4,
        validity_judgement: 1,
        parse_error: String::from(""),
        token_id: token_id.to_vec(),
        burn_flags: vec![],
        tx_metadata: Some(tx_metadata) 
    };

    let input = Input{
        index: 0,
        // outpoint: None,
        outpoint: Some(Outpoint{ hash: generate_random_sha256()?, index: 0 }),
        signature_script: vec![],
        sequence: 0,
        value: 123456,
        previous_script: vec![],
        address: String::from(""),
        slp_token: None
    };

    let input2 = Input{
        index: 1,
        // outpoint: None,
        outpoint: Some(Outpoint{ hash: generate_random_sha256()?, index: 0 }),
        signature_script: vec![],
        sequence: 0,
        value: 123456,
        previous_script: vec![],
        address: String::from(""),
        slp_token: Some(slp_token.clone())
    };

    let output = Output{
        index: 0,
        value: 123456,
        pubkey_script: generate_script()?,
        address: String::from(""),
        script_class: String::from(""),
        disassembled_script: String::from(""),
        slp_token: Some(slp_token.clone())
    };

    Ok(Transaction{
        hash: generate_random_sha256()?,
        version: 0,
        inputs: vec![input, input2],
        outputs: vec![output],
        lock_time: 0,
        size: 123456,
        timestamp: 1636076932,
        confirmations: 1,
        block_height: height,
        block_hash: block_hash.to_vec(),
        slp_transaction_info: Some(slp_transaction_info)
    })
}


pub fn generate_transaction_data(transaction: Transaction) -> TransactionData {
    let txids_or_txs = TxidsOrTxs::Transaction(transaction);

    return TransactionData{
        txids_or_txs: Some(txids_or_txs)
    }
}

pub fn generate_block(height: i32, block_hash: &Vec<u8>, transactions: ::std::vec::Vec<TransactionData>) -> Result<Block> {
    let block_info = BlockInfo{
        hash: block_hash.to_vec(),
        height: height,
        version: 0,
        previous_block: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        merkle_root: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        timestamp: 1636076932,
        bits: 2,
        nonce: 123456,
        confirmations: height,
        difficulty: 1.00,
        next_block_hash: vec![],
        size: 123456,
        median_time: 123456
    };

    Ok(Block{
        info: Some(block_info),
        transaction_data: transactions
    })
}

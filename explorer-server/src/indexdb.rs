use std::{collections::HashMap, path::Path};
use std::convert::TryInto;

use anyhow::{Context, Result, anyhow, bail};
use serde::de::DeserializeOwned;
use rocksdb::{ColumnFamily, Options, WriteBatch};
use zerocopy::{AsBytes, FromBytes, U32, Unaligned};
use bitcoin_cash::{Address, Hashed};
use byteorder::BE;

use crate::{blockchain::{Destination, destination_from_script, from_le_hex, is_coinbase, to_le_hex}, grpc::bchrpc, primitives::{AddressTx, BlockMeta, SlpAction, TokenMeta, TxMeta, TxMetaVariant, Utxo}};

pub struct IndexDb {
    db: rocksdb::DB,
}

pub struct BlockBatches {
    pub block_height: i32,
    batch: WriteBatch,
}

#[derive(FromBytes, AsBytes, Unaligned, Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct AddrTxKey {
    pub addr_type: u8,
    pub addr_hash: [u8; 20],
    pub block_height: U32<BE>,
    pub tx_hash: [u8; 32],
}

#[derive(FromBytes, AsBytes, Unaligned, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct AddrKeyPrefix {
    pub addr_type: u8,
    pub addr_hash: [u8; 20],
}

#[derive(FromBytes, AsBytes, Unaligned, Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct UtxoKey {
    pub tx_hash: [u8; 32],
    pub out_idx: U32<BE>,
}

#[derive(FromBytes, AsBytes, Unaligned, Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct AddrUtxoKey {
    pub addr: AddrKeyPrefix,
    pub utxo_key: UtxoKey,
}

#[derive(FromBytes, AsBytes, Unaligned, Debug, Default)]
#[repr(C)]
pub struct TxOutSpend {
    pub by_tx_hash: [u8; 32],
    pub by_tx_input_idx: U32<BE>,
}

pub struct AddressBalance {
    pub utxos: HashMap<Option<[u8; 32]>, Vec<(UtxoKey, Utxo)>>,
    pub balances: HashMap<Option<[u8; 32]>, (i64, u64)>,
}

impl IndexDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut db;
        if path.as_ref().exists() {
            let cfs = rocksdb::DB::list_cf(&Options::default(), &path)?;
            db = rocksdb::DB::open_cf(&Options::default(), &path, cfs)?;
        } else {
            db = rocksdb::DB::open_default(&path)?;
        }
        Self::ensure_cf(&mut db, "block_height_idx")?;
        Self::ensure_cf(&mut db, "block_meta")?;
        Self::ensure_cf(&mut db, "tx_meta")?;
        Self::ensure_cf(&mut db, "addr_tx_meta")?;
        Self::ensure_cf(&mut db, "addr_utxo")?;
        Self::ensure_cf(&mut db, "utxo_set")?;
        Self::ensure_cf(&mut db, "tx_out_spend")?;
        Self::ensure_cf(&mut db, "token_meta")?;

        Self::ensure_cf(&mut db, "mempool_tx_meta")?;
        Self::ensure_cf(&mut db, "mempool_addr_tx_meta")?;
        Self::ensure_cf(&mut db, "mempool_addr_utxo_add")?;
        Self::ensure_cf(&mut db, "mempool_addr_utxo_remove")?;
        Self::ensure_cf(&mut db, "mempool_utxo_set_add")?;
        Self::ensure_cf(&mut db, "mempool_utxo_set_remove")?;
        Self::ensure_cf(&mut db, "mempool_tx_out_spend")?;
        Self::ensure_cf(&mut db, "mempool_token_meta")?;

        Ok(IndexDb {
            db,
        })
    }

    fn ensure_cf(db: &mut rocksdb::DB, name: &str) -> Result<()> {
        if let None = db.cf_handle(name) {
            db.create_cf(name, &Options::default())?;
        }
        Ok(())
    }

    fn cf_block_height_idx(&self) -> &ColumnFamily {
        self.db.cf_handle("block_height_idx").expect("No such column family")
    }
    fn cf_block_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("block_meta").expect("No such column family")
    }
    fn cf_tx_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("tx_meta").expect("No such column family")
    }
    fn cf_addr_tx_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("addr_tx_meta").expect("No such column family")
    }
    fn cf_addr_utxo(&self) -> &ColumnFamily {
        self.db.cf_handle("addr_utxo").expect("No such column family")
    }
    fn cf_utxo_set(&self) -> &ColumnFamily {
        self.db.cf_handle("utxo_set").expect("No such column family")
    }
    fn cf_tx_out_spend(&self) -> &ColumnFamily {
        self.db.cf_handle("tx_out_spend").expect("No such column family")
    }
    fn cf_token_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("token_meta").expect("No such column family")
    }

    fn cf_mempool_tx_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_tx_meta").expect("No such column family")
    }
    fn cf_mempool_addr_tx_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_addr_tx_meta").expect("No such column family")
    }
    fn cf_mempool_addr_utxo_add(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_addr_utxo_add").expect("No such column family")
    }
    fn cf_mempool_addr_utxo_remove(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_addr_utxo_remove").expect("No such column family")
    }
    fn cf_mempool_utxo_set_add(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_utxo_set_add").expect("No such column family")
    }
    fn cf_mempool_utxo_set_remove(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_utxo_set_remove").expect("No such column family")
    }
    fn cf_mempool_tx_out_spend(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_tx_out_spend").expect("No such column family")
    }
    fn cf_mempool_token_meta(&self) -> &ColumnFamily {
        self.db.cf_handle("mempool_token_meta").expect("No such column family")
    }

    pub fn last_block_height(&self) -> Result<u32> {
        let mut iterator = self.db.raw_iterator_cf(self.cf_block_height_idx());
        iterator.seek_to_last();
        match iterator.key() {
            Some(key) => {
                let key: [u8; 4] = key.try_into()?;
                let height = u32::from_be_bytes(key);
                Ok(height)
            }
            None => Ok(0)
        }
    }

    pub fn block_range(&self, start_height: u32, num_blocks: u32) -> Result<Vec<([u8; 32], BlockMeta)>> {
        let start_key = start_height.to_be_bytes();
        let mut block_metas = Vec::with_capacity(num_blocks as usize);
        let mut iterator = self.db.raw_iterator_cf(self.cf_block_height_idx());
        iterator.seek(&start_key);
        let cf_block_meta = self.cf_block_meta();
        for _ in 0..num_blocks {
            if let Some(value) = iterator.value() {
                let hash: [u8; 32] = value.try_into()?;
                let block_meta: BlockMeta = self.db_get(cf_block_meta, &hash)?;
                block_metas.push((hash, block_meta));
            } else {
                break;
            }
            iterator.next();
        }
        Ok(block_metas)
    }

    pub fn block_hash_at(&self, height: u32) -> Result<Option<[u8; 32]>> {
        let height_key = height.to_be_bytes();
        let block_hash = self.db.get_cf(self.cf_block_height_idx(), height_key)?;
        let block_hash = block_hash.map(|block_hash| block_hash.as_slice().try_into()).transpose()?;
        Ok(block_hash)
    }

    pub fn block_meta(&self, block_hash: &[u8]) -> Result<Option<BlockMeta>> {
        self.db_get_option(self.cf_block_meta(), block_hash)
    }

    pub fn tx_meta(&self, tx_hash: &[u8]) -> Result<Option<TxMeta>> {
        match self.db_get_option(self.cf_mempool_tx_meta(), tx_hash)? {
            Some(tx) => Ok(Some(tx)),
            None => self.db_get_option(self.cf_tx_meta(), tx_hash)
        }
    }

    pub fn token_meta(&self, token_id: &[u8]) -> Result<Option<TokenMeta>> {
        match self.db_get_option(self.cf_mempool_token_meta(), token_id)? {
            Some(tx) => Ok(Some(tx)),
            None => self.db_get_option(self.cf_token_meta(), token_id)
        }
    }

    pub fn tx_out_spends(&self, tx_hash: &[u8]) -> Result<HashMap<u32, Option<TxOutSpend>>> {
        let mut spends = HashMap::new();
        for &cf in &[self.cf_mempool_utxo_set_add(), self.cf_utxo_set()] {
            let mut iter_utxos = self.db.raw_iterator_cf(cf);
            iter_utxos.seek(tx_hash);
            while let Some(key) = iter_utxos.key() {
                let mut utxo_key = UtxoKey::default();
                utxo_key.as_bytes_mut().copy_from_slice(&key);
                if &utxo_key.tx_hash != tx_hash {
                    break;
                }
                spends.insert(utxo_key.out_idx.get(), None);
                iter_utxos.next();
            }
        }
        for &cf in &[self.cf_mempool_tx_out_spend(), self.cf_tx_out_spend()] {
            let mut iter_spends = self.db.raw_iterator_cf(cf);
            iter_spends.seek(tx_hash);
            while let (Some(key), Some(value)) = (iter_spends.key(), iter_spends.value()) {
                let mut utxo_key = UtxoKey::default();
                utxo_key.as_bytes_mut().copy_from_slice(&key);
                if &utxo_key.tx_hash != tx_hash {
                    break;
                }
                let mut tx_out_spend = TxOutSpend::default();
                tx_out_spend.as_bytes_mut().copy_from_slice(&value);
                spends.insert(utxo_key.out_idx.get(), Some(tx_out_spend));
                iter_spends.next();
            }
        }
        Ok(spends)
    }

    pub fn address(&self, address: &Address<'_>, skip: usize, take: usize) -> Result<Vec<([u8; 32], AddressTx, TxMeta)>> {
        let addr_prefix = AddrKeyPrefix {
            addr_type: address.addr_type() as u8,
            addr_hash: address.hash().as_slice().try_into().unwrap(),
        };
        let mut entries = Vec::new();

        let mut iter_mempool_addr_tx = self.db.raw_iterator_cf(self.cf_mempool_addr_tx_meta());
        iter_mempool_addr_tx.seek(addr_prefix.as_bytes());
        for _ in 0..skip {
            if !iter_mempool_addr_tx.valid() {
                break;
            }
            iter_mempool_addr_tx.next();
        }
        let mut n = 0;
        while let (Some(key), Some(value)) = (iter_mempool_addr_tx.key(), iter_mempool_addr_tx.value()) {
            if n >= take {
                break;
            }
            let mut addr_tx_key = AddrTxKey::default();
            addr_tx_key.as_bytes_mut().copy_from_slice(&key);
            if addr_tx_key.addr_hash != addr_prefix.addr_hash {
                break;
            }
            let address_tx: AddressTx = bincode::deserialize(&value)?;
            let tx_meta = self.tx_meta(&addr_tx_key.tx_hash)?.ok_or_else(|| anyhow!("No tx meta"))?;
            entries.push((addr_tx_key.tx_hash, address_tx, tx_meta));
            iter_mempool_addr_tx.next();
            n += 1;
        }

        let mut iter_addr_tx = self.db.raw_iterator_cf(self.cf_addr_tx_meta());
        let mut seek_key = addr_prefix.as_bytes().to_vec();
        inc_bytes(&mut seek_key);
        iter_addr_tx.seek(seek_key);
        iter_addr_tx.prev();
        for _ in n..skip {
            if !iter_addr_tx.valid() {
                break;
            }
            iter_addr_tx.prev();
        }
        while let (Some(key), Some(value)) = (iter_addr_tx.key(), iter_addr_tx.value()) {
            if n >= take {
                break;
            }
            let mut addr_tx_key = AddrTxKey::default();
            addr_tx_key.as_bytes_mut().copy_from_slice(&key);
            if addr_tx_key.addr_hash != addr_prefix.addr_hash {
                break;
            }
            let address_tx: AddressTx = bincode::deserialize(&value)?;
            let tx_meta = self.tx_meta(&addr_tx_key.tx_hash)?.ok_or_else(|| anyhow!("No tx meta"))?;
            entries.push((addr_tx_key.tx_hash, address_tx, tx_meta));
            iter_addr_tx.prev();
            n += 1;
        }
        Ok(entries)
    }

    pub fn address_num_txs(&self, address: &Address<'_>) -> Result<usize> {
        let addr_prefix = AddrKeyPrefix {
            addr_type: address.addr_type() as u8,
            addr_hash: address.hash().as_slice().try_into().unwrap(),
        };
        let mut n = 0;
        for &cf in &[self.cf_mempool_addr_tx_meta(), self.cf_addr_tx_meta()] {
            let mut iter_addr_tx = self.db.raw_iterator_cf(cf);
            iter_addr_tx.seek(addr_prefix.as_bytes());
            while let Some(key) = iter_addr_tx.key() {
                let mut addr_tx_key = AddrTxKey::default();
                addr_tx_key.as_bytes_mut().copy_from_slice(&key);
                if addr_tx_key.addr_hash != addr_prefix.addr_hash {
                    break;
                }
                n += 1;
                iter_addr_tx.next();
            }
        }
        Ok(n)
    }

    pub fn utxo(&self, utxo_key: &UtxoKey) -> Result<Option<Utxo>> {
        if let Some(_) = self.db.get_cf(self.cf_mempool_utxo_set_remove(), utxo_key.as_bytes())? {
            return Ok(None);
        }
        match self.db_get_option(self.cf_mempool_utxo_set_add(), utxo_key.as_bytes())? {
            Some(tx) => Ok(Some(tx)),
            None => self.db_get_option(self.cf_utxo_set(), utxo_key.as_bytes())
        }
    }

    pub fn address_balance(&self, sats_address: &Address<'_>, _skip: usize, _take: usize) -> Result<AddressBalance> {
        let mut utxos = HashMap::new();
        let mut balances = HashMap::new();
        let addr_prefix = AddrKeyPrefix {
            addr_type: sats_address.addr_type() as u8,
            addr_hash: sats_address.hash().as_slice().try_into().unwrap(),
        };
        utxos.insert(None, vec![]);
        balances.insert(None, (0, 0));
        for &cf in &[self.cf_mempool_addr_utxo_add(), self.cf_addr_utxo()] {
            let mut iter_addr_utxo = self.db.raw_iterator_cf(cf);
            iter_addr_utxo.seek(addr_prefix.as_bytes());
            while let Some(key) = iter_addr_utxo.key() {
                let mut addr_utxo_key = AddrUtxoKey::default();
                addr_utxo_key.as_bytes_mut().copy_from_slice(&key);
                if addr_utxo_key.addr != addr_prefix {
                    break;
                }
                if let Some(utxo) = self.utxo(&addr_utxo_key.utxo_key)? {
                    let token_utxos = utxos.entry(utxo.token_id).or_insert(vec![]);
                    let (balance_sats, balance_token) = balances.entry(utxo.token_id).or_insert((0, 0));
                    *balance_sats += utxo.sats_amount;
                    *balance_token += utxo.token_amount;
                    token_utxos.push((addr_utxo_key.utxo_key, utxo));
                }
                iter_addr_utxo.next();
            }
        }
        Ok(AddressBalance { utxos, balances })
    }

    pub fn search(&self, query: &str) -> Result<Option<String>> {
        match Address::from_cash_addr(query) {
            Ok(address) => return Ok(Some(format!("/address/{}", address.cash_addr()))),
            _ => {},
        }
        let bytes = from_le_hex(query)?;
        match self.tx_meta(&bytes)? {
            Some(_) => return Ok(Some(format!("/tx/{}", query))),
            _ => {}
        }
        let block_height: u32 = query.parse().unwrap_or_default();
        if block_height == 0 {
            match self.block_meta(&bytes) {
                Ok(_) => return Ok(Some(format!("/block/{}", query))),
                _ => {}
            }
        } else {
            match self.block_hash_at(block_height)? {
                Some(_) => return Ok(Some(format!("/block-height/{}", block_height))),
                _ => {}
            }
        }
        Ok(None)
    }

    pub fn apply_block_batches(&self, block_batches: BlockBatches) -> Result<()> {
        Ok(self.db.write(block_batches.batch)?)
    }

    pub fn apply_batch(&self, batch: WriteBatch) -> Result<()> {
        Ok(self.db.write(batch)?)
    }

    pub fn clear_mempool(&self) -> Result<()> {
        self.clear_cf(self.cf_mempool_tx_meta())?;
        self.clear_cf(self.cf_mempool_addr_tx_meta())?;
        self.clear_cf(self.cf_mempool_addr_utxo_add())?;
        self.clear_cf(self.cf_mempool_addr_utxo_remove())?;
        self.clear_cf(self.cf_mempool_utxo_set_add())?;
        self.clear_cf(self.cf_mempool_utxo_set_remove())?;
        self.clear_cf(self.cf_mempool_tx_out_spend())?;
        self.clear_cf(self.cf_mempool_token_meta())?;
        Ok(())
    }

    fn clear_cf(&self, cf: &ColumnFamily) -> Result<()> {
        self.db.delete_range_cf(cf, b"".as_ref(), &[0xff; 512])?;
        Ok(())
    }

    pub fn make_block_batches(&self, block: &bchrpc::Block) -> Result<BlockBatches> {
        use bchrpc::block::transaction_data::TxidsOrTxs;
        let block_info = block.info.as_ref().ok_or_else(|| anyhow!("No block info"))?;
        let txs = block.transaction_data.iter()
            .map(|tx_data| {
                match &tx_data.txids_or_txs {
                    Some(TxidsOrTxs::Transaction(tx)) => Ok(tx),
                    _ => bail!("Invalid tx in handle_block"),
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| "Collecting transactions")?;
        let mut batch = WriteBatch::default();
        self.add_block_height_idx(&mut batch, block_info);
        self.add_block_meta(&mut batch, block_info, &txs).with_context(|| "add_block_meta")?;
        self.update_addr_utxo_set(&mut batch, &txs, false).with_context(|| "update_addr_utxo_set")?;
        self.update_utxo_set(&mut batch, &txs, false).with_context(|| "update_utxo_set")?;
        for tx in txs {
            self.add_tx_meta(&mut batch, tx, false).with_context(|| "add_tx_meta")?;
            self.add_addr_tx_meta(&mut batch, tx, false).with_context(|| "add_addr_tx_meta")?;
            self.add_tx_out_spend(&mut batch, tx, false).with_context(|| "add_tx_out_spend")?;
            self.add_token_meta(&mut batch, tx, false).with_context(|| "add_token_meta")?;
        }
        Ok(BlockBatches {
            block_height: block_info.height,
            batch,
        })
    }

    pub fn make_mempool_tx_batches(&self, txs: &[&bchrpc::Transaction]) -> Result<WriteBatch> {
        let mut batch = WriteBatch::default();
        self.update_addr_utxo_set(&mut batch, &txs, true).with_context(|| "update_addr_utxo_set")?;
        self.update_utxo_set(&mut batch, &txs, true).with_context(|| "update_utxo_set")?;
        for tx in txs {
            self.add_tx_meta(&mut batch, tx, true).with_context(|| "add_tx_meta")?;
            self.add_addr_tx_meta(&mut batch, tx, true).with_context(|| "add_addr_tx_meta")?;
            self.add_tx_out_spend(&mut batch, tx, true).with_context(|| "add_tx_out_spend")?;
            self.add_token_meta(&mut batch, tx, true).with_context(|| "add_token_meta")?;
        }
        Ok(batch)
    }

    pub fn make_mempool_txs<'a>(&self, txs: &'a [bchrpc::get_mempool_response::TransactionData]) -> Result<Vec<&'a bchrpc::Transaction>> {
        use bchrpc::get_mempool_response::transaction_data::TxidsOrTxs;
        let txs = txs.iter()
            .map(|tx_data| {
                match &tx_data.txids_or_txs {
                    Some(TxidsOrTxs::Transaction(tx)) => Ok(tx),
                    _ => bail!("Invalid tx in handle_block"),
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| "Collecting transactions")?;
        Ok(txs)
    }

    pub fn flush(&self) -> Result<()> {
        self.db.flush()?;
        Ok(())
    }

    fn add_block_height_idx(&self, batch: &mut WriteBatch, block_info: &bchrpc::BlockInfo) {
        let block_height = block_info.height as u32;
        batch.put_cf(self.cf_block_height_idx(), block_height.to_be_bytes(), &block_info.hash);
    }

    fn add_block_meta(&self, batch: &mut WriteBatch, block_info: &bchrpc::BlockInfo, txs: &[&bchrpc::Transaction]) -> Result<()> {
        let mut total_sats_input = 0;
        let mut total_sats_output = 0;
        for tx in txs {
            for input in &tx.inputs {
                total_sats_input += input.value;
            }
            for output in &tx.outputs {
                total_sats_output += output.value;
            }
        }
        let coinbase_data = txs[0].inputs[0].signature_script.clone();
        let block_meta = BlockMeta {
            height: block_info.height,
        
            version: block_info.version,
            previous_block: block_info.previous_block.as_slice().try_into()?,
            merkle_root: block_info.merkle_root.as_slice().try_into()?,
            timestamp: block_info.timestamp,
            bits: block_info.bits,
            nonce: block_info.nonce,
        
            total_sats_input,
            total_sats_output,
        
            difficulty: block_info.difficulty,
            median_time: block_info.median_time,
            size: block_info.size as u64,
            num_txs: txs.len() as u64,
            coinbase_data,
        };
        batch.put_cf(self.cf_block_meta(), block_info.hash.clone(), bincode::serialize(&block_meta)?);
        Ok(())
    }

    fn add_tx_meta(&self, batch: &mut WriteBatch, tx: &bchrpc::Transaction, is_mempool: bool) -> Result<()> {
        let cf = if is_mempool { self.cf_mempool_tx_meta() } else { self.cf_tx_meta() };
        let outpoint = tx.inputs.get(0).ok_or_else(|| anyhow!("No input"))?.outpoint.as_ref().ok_or_else(|| anyhow!("No outpoint"))?;
        let tx_meta = TxMeta {
            block_height: tx.block_height,
            timestamp: tx.timestamp,
            is_coinbase: is_coinbase(outpoint),
            size: tx.size,
            num_inputs: tx.inputs.len() as u32,
            num_outputs: tx.outputs.len() as u32,
            sats_input: tx.inputs.iter().map(|input| input.value).sum(),
            sats_output: tx.outputs.iter().map(|output| output.value).sum(),
            variant: Self::tx_meta_variant(tx),
        };
        batch.put_cf(cf, tx.hash.as_slice(), bincode::serialize(&tx_meta)?);
        Ok(())
    }

    fn tx_meta_variant(tx: &bchrpc::Transaction) -> TxMetaVariant {
        use bchrpc::{slp_transaction_info::ValidityJudgement};
        match &tx.slp_transaction_info {
            Some(slp) => {
                let input_sum: u64 = tx.inputs
                    .iter()
                    .map(|input| input.slp_token.as_ref().map(|token| token.amount).unwrap_or_default())
                    .sum();
                let output_sum: u64 = tx.outputs
                    .iter()
                    .map(|output| output.slp_token.as_ref().map(|token| token.amount).unwrap_or_default())
                    .sum();
                if slp.validity_judgement() == ValidityJudgement::UnknownOrInvalid {
                    if input_sum == 0 {
                        return TxMetaVariant::SatsOnly;
                    } else {
                        return TxMetaVariant::InvalidSlp {
                            token_id: slp.token_id.clone(),
                            token_input: input_sum,
                        };
                    }
                }
                TxMetaVariant::Slp {
                    action: {
                        use bchrpc::SlpAction::*;
                        match slp.slp_action() {
                            NonSlp => return TxMetaVariant::SatsOnly,
                            NonSlpBurn | SlpParseError | SlpUnsupportedVersion => return TxMetaVariant::InvalidSlp {
                                token_id: slp.token_id.as_slice().try_into().unwrap(),
                                token_input: input_sum,
                            },
                            SlpV1Genesis => SlpAction::SlpV1Genesis,
                            SlpV1Mint => SlpAction::SlpV1Mint,
                            SlpV1Send => SlpAction::SlpV1Send,
                            SlpV1Nft1GroupGenesis => SlpAction::SlpV1Nft1GroupGenesis,
                            SlpV1Nft1GroupMint => SlpAction::SlpV1Nft1GroupMint,
                            SlpV1Nft1GroupSend => SlpAction::SlpV1Nft1GroupSend,
                            SlpV1Nft1UniqueChildGenesis => SlpAction::SlpV1Nft1UniqueChildGenesis,
                            SlpV1Nft1UniqueChildSend => SlpAction::SlpV1Nft1UniqueChildSend,
                        }
                    },
                    token_input: input_sum,
                    token_output: output_sum,
                    token_id: slp.token_id.as_slice().try_into().unwrap(),
                }
            }
            None => TxMetaVariant::SatsOnly
        }
    }

    fn add_addr_tx_meta(&self, batch: &mut WriteBatch, tx: &bchrpc::Transaction, is_mempool: bool) -> Result<()> {
        let cf = if is_mempool { self.cf_mempool_addr_tx_meta() } else { self.cf_addr_tx_meta() };
        let mut address_delta = HashMap::new();
        for input in &tx.inputs {
            let (delta_sats, delta_tokens) = address_delta.entry(input.previous_script.as_slice()).or_default();
            *delta_sats -= input.value;
            if let Some(slp) = &input.slp_token {
                *delta_tokens -= slp.amount as i64;
            }
        }
        for output in &tx.outputs {
            let (delta_sats, delta_tokens) = address_delta.entry(output.pubkey_script.as_slice()).or_default();
            *delta_sats += output.value;
            if let Some(slp) = &output.slp_token {
                *delta_tokens += slp.amount as i64;
            }
        }
        for (pubkey_script, (delta_sats, delta_tokens)) in address_delta {
            let destination = destination_from_script("abc", pubkey_script);
            if let Destination::Address(address) = destination {
                let addr_tx_key = AddrTxKey {
                    addr_type: address.addr_type() as u8,
                    addr_hash: address.hash().as_slice().try_into()?,
                    block_height: U32::new(tx.block_height as u32),
                    tx_hash: tx.hash.as_slice().try_into()?,
                };
                let addr_tx = AddressTx {
                    timestamp: tx.timestamp,
                    block_height: tx.block_height,
                    delta_sats,
                    delta_tokens,
                };
                batch.put_cf(cf, addr_tx_key.as_bytes(), bincode::serialize(&addr_tx)?);
            }
        }
        Ok(())
    }

    fn update_utxo_set(&self, batch: &mut WriteBatch, txs: &[&bchrpc::Transaction], is_mempool: bool) -> Result<()> {
        let cf_add = if is_mempool { self.cf_mempool_utxo_set_add() } else { self.cf_utxo_set() };
        for tx in txs {
            let tx_hash: [u8; 32] = tx.hash.as_slice().try_into()?;
            let token_id: Option<[u8; 32]> = match &tx.slp_transaction_info {
                Some(slp) if !slp.token_id.is_empty() => Some(slp.token_id[..].try_into()?),
                _ => None,
            };
            let outpoint = tx.inputs.get(0).ok_or_else(|| anyhow!("No input"))?.outpoint.as_ref().ok_or_else(|| anyhow!("No outpoint"))?;
            for (out_idx, output) in tx.outputs.iter().enumerate() {
                let utxo_key = UtxoKey {
                    tx_hash,
                    out_idx: U32::new(out_idx as u32),
                };
                let utxo = Utxo {
                    sats_amount: output.value,
                    token_amount: output.slp_token.as_ref().map(|slp| slp.amount).unwrap_or(0),
                    is_coinbase: is_coinbase(outpoint),
                    block_height: tx.block_height,
                    token_id,
                };
                batch.put_cf(cf_add, utxo_key.as_bytes(), bincode::serialize(&utxo)?);
            }
        }
        for tx in txs {
            for input in &tx.inputs {
                if let Some(outpoint) = &input.outpoint {
                    let utxo_key = UtxoKey {
                        tx_hash: outpoint.hash.as_slice().try_into()?,
                        out_idx: U32::new(outpoint.index),
                    };
                    if is_mempool {
                        batch.put_cf(self.cf_mempool_utxo_set_remove(), utxo_key.as_bytes(), b"");
                    } else {
                        batch.delete_cf(self.cf_utxo_set(), utxo_key.as_bytes());
                    };
                }
            }
        }
        Ok(())
    }

    fn update_addr_utxo_set(&self, batch: &mut WriteBatch, txs: &[&bchrpc::Transaction], is_mempool: bool) -> Result<()> {
        let cf_add = if is_mempool { self.cf_mempool_addr_utxo_add() } else { self.cf_addr_utxo() };
        for tx in txs {
            let tx_hash: [u8; 32] = tx.hash.as_slice().try_into()?;
            for (out_idx, output) in tx.outputs.iter().enumerate() {
                if let Destination::Address(address) = destination_from_script("abc", &output.pubkey_script) {
                    let key = AddrUtxoKey {
                        addr: AddrKeyPrefix {
                            addr_type: address.addr_type() as u8,
                            addr_hash: address.hash().as_slice().try_into()?,
                        },
                        utxo_key: UtxoKey {
                            tx_hash,
                            out_idx: U32::new(out_idx as u32),
                        },
                    };
                    batch.put_cf(cf_add, key.as_bytes(), b"");
                }
            }
        }
        for tx in txs {
            for input in &tx.inputs {
                if let Destination::Address(address) = destination_from_script("abc", &input.previous_script) {
                    if let Some(outpoint) = &input.outpoint {
                        let key = AddrUtxoKey {
                            addr: AddrKeyPrefix {
                                addr_type: address.addr_type() as u8,
                                addr_hash: address.hash().as_slice().try_into()?,
                            },
                            utxo_key: UtxoKey {
                                tx_hash: outpoint.hash.as_slice().try_into()?,
                                out_idx: U32::new(outpoint.index),
                            },
                        };
                        if is_mempool {
                            batch.put_cf(self.cf_mempool_addr_utxo_remove(), key.as_bytes(), b"");
                        } else {
                            batch.delete_cf(self.cf_addr_utxo(), key.as_bytes());
                        };
                    }
                }
            }
        }
        Ok(())
    }

    fn add_tx_out_spend(&self, batch: &mut WriteBatch, tx: &bchrpc::Transaction, is_mempool: bool) -> Result<()> {
        let cf = if is_mempool { self.cf_mempool_tx_out_spend() } else { self.cf_tx_out_spend() };
        let by_tx_hash: [u8; 32] = tx.hash.as_slice().try_into()?;
        for (input_idx, input) in tx.inputs.iter().enumerate() {
            if let Some(outpoint) = &input.outpoint {
                let utxo_key = UtxoKey {
                    tx_hash: outpoint.hash.as_slice().try_into()?,
                    out_idx: U32::new(outpoint.index),
                };
                let spend = TxOutSpend {
                    by_tx_hash,
                    by_tx_input_idx: U32::new(input_idx as u32),
                };
                batch.put_cf(cf, utxo_key.as_bytes(), spend.as_bytes());
            }
        }
        Ok(())
    }

    fn add_token_meta(&self, batch: &mut WriteBatch, tx: &bchrpc::Transaction, is_mempool: bool) -> Result<()> {
        let cf = if is_mempool { self.cf_mempool_token_meta() } else { self.cf_token_meta() };
        use bchrpc::{SlpAction, slp_transaction_info::{TxMetadata, ValidityJudgement}};
        let slp = match &tx.slp_transaction_info {
            Some(slp) if !slp.token_id.is_empty() => slp,
            _ => return Ok(()),
        };
        if slp.validity_judgement() != ValidityJudgement::Valid {
            return Ok(());
        }
        let token_meta = match (slp.slp_action(), &slp.tx_metadata) {
            (SlpAction::SlpV1Genesis, Some(TxMetadata::V1Genesis(genesis))) => {
                TokenMeta {
                    token_type: 0x01,
                    token_ticker: genesis.ticker.clone(),
                    token_name: genesis.name.clone(),
                    token_document_url: genesis.document_url.clone(),
                    token_document_hash: genesis.document_hash.clone(),
                    decimals: genesis.decimals,
                    group_id: None,
                }
            },
            (SlpAction::SlpV1Nft1GroupGenesis, Some(TxMetadata::V1Genesis(genesis))) => {
                TokenMeta {
                    token_type: 0x81,
                    token_ticker: genesis.ticker.clone(),
                    token_name: genesis.name.clone(),
                    token_document_url: genesis.document_url.clone(),
                    token_document_hash: genesis.document_hash.clone(),
                    decimals: genesis.decimals,
                    group_id: None,
                }
            },
            (SlpAction::SlpV1Nft1UniqueChildGenesis, Some(TxMetadata::V1Nft1ChildGenesis(genesis))) => {
                TokenMeta {
                    token_type: 0x41,
                    token_ticker: genesis.ticker.clone(),
                    token_name: genesis.name.clone(),
                    token_document_url: genesis.document_url.clone(),
                    token_document_hash: genesis.document_hash.clone(),
                    decimals: genesis.decimals,
                    group_id: {
                        let group_id = genesis.group_token_id.as_slice().try_into()
                            .with_context(|| format!("Invalid group token id: {}, for tx {}", hex::encode(&genesis.group_token_id), to_le_hex(&tx.hash)))
                            .unwrap_or_else(|err| {
                                println!("Invalid group_token_id: {:?}", err);
                                [0; 32]
                            });
                        Some(group_id)
                    },
                }
            },
            _ => return Ok(()),
        };
        batch.put_cf(cf, slp.token_id.as_slice(), bincode::serialize(&token_meta)?);
        Ok(())
    }

    fn db_get<T: DeserializeOwned>(&self, cf: &ColumnFamily, key: &[u8]) -> Result<T> {
        let item = self.db
            .get_cf(cf, key)?
            .ok_or_else(|| anyhow!("No entry for {}", String::from_utf8_lossy(key)))?;
        Ok(bincode::deserialize(&item)?)
    }
    
    fn db_get_option<T: DeserializeOwned>(&self, cf: &ColumnFamily, key: &[u8]) -> Result<Option<T>> {
        let item = self.db.get_cf(cf, key)?;
        Ok(item
            .map(|item| bincode::deserialize(&item))
            .transpose()?)
    }
}

fn inc_bytes(bytes: &mut [u8]) {
    for byte in bytes.iter_mut().rev() {
        *byte = byte.wrapping_add(1);
        if *byte != 0 {
            return;
        }
    }
}

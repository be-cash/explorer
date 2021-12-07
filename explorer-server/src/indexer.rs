use std::{collections::HashMap, convert::TryInto, sync::{Arc, atomic::{AtomicUsize, AtomicBool, Ordering}}, time::Instant};

use anyhow::{Result, anyhow, bail};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint};
use tokio::sync::{mpsc, watch};
use crate::{blockchain::to_le_hex, grpc::bchrpc, indexdb::{BlockBatches, IndexDb, TxOutSpend}, primitives::{TokenMeta, TxMeta, TxMetaVariant}};
use crate::{mocker};
use crate::grpc::bchrpc::bchrpc_client::BchrpcClient;
use async_trait::async_trait;
use ctrlc;


const ALPN_H2: &'static str = "h2";

#[async_trait]
pub trait Indexer: Sync + Send {
    async fn connect(db: IndexDb) -> Result<Self> where Self: Sized;
    fn db(&self) -> &IndexDb;
    async fn block_txs(&self, block_hash: &[u8]) -> Result<Vec<([u8; 32], TxMeta)>>;
    async fn tx(&self, tx_hash: &[u8]) -> Result<Tx>;
    async fn run_indexer(self: Arc<Self>);
    async fn run_indexer_inner(self: Arc<Self>) -> Result<()>;
    async fn index_thread(
        &self,
        current_height_atomic: Arc<AtomicUsize>,
        mut send_batches: mpsc::Sender<BlockBatches>,
        mut watch_height_receiver: watch::Receiver<usize>,
    ) -> Result<()>;
    async fn monitor_new_blocks(&self);
    async fn try_monitor_new_blocks(&self) -> Result<()>;
    async fn monitor_mempool(&self);
    async fn try_monitor_mempool(&self) -> Result<()>;
    async fn update_mempool(&self) -> Result<()>;
}

pub struct IndexerProduction {
    db: IndexDb,
    bchd: BchrpcClient<Channel>,
    max_fetch_ahead: usize,
}

pub struct IndexerDevelopment {
    db: IndexDb,
    max_fetch_ahead: usize,
    scheduled_for_termination: Arc<AtomicBool>
}

pub struct Tx {
    pub transaction: bchrpc::Transaction,
    pub tx_meta: TxMeta,
    pub token_meta: Option<TokenMeta>,
    pub raw_tx: Vec<u8>,
    pub tx_out_spends: HashMap<u32, Option<TxOutSpend>>,
}

struct NopCertVerifier;

impl tokio_rustls::rustls::ServerCertVerifier for NopCertVerifier {
    fn verify_server_cert(
        &self,
        _roots: & tokio_rustls::rustls::RootCertStore,
        _presented_certs: &[ tokio_rustls::rustls::Certificate],
        _dns_name: webpki::DNSNameRef,
        _ocsp_response: &[u8],
    ) -> Result< tokio_rustls::rustls::ServerCertVerified,  tokio_rustls::rustls::TLSError> {
        Ok( tokio_rustls::rustls::ServerCertVerified::assertion())
    }
}

#[async_trait]
impl Indexer for IndexerProduction {
    async fn connect(db: IndexDb) -> Result<Self> {
        const MAX_FETCH_AHEAD: usize = 1000;
        use std::fs;
        use std::io::Read;
        let mut cert_file = fs::File::open("cert.crt")?;
        let mut cert = Vec::new();
        cert_file.read_to_end(&mut cert)?;
        let mut config =  tokio_rustls::rustls::ClientConfig::new();
        config.set_protocols(&[Vec::from(&ALPN_H2[..])]);
        let mut dangerous_config =  tokio_rustls::rustls::DangerousClientConfig {
            cfg: &mut config,
        };
        dangerous_config.set_certificate_verifier(Arc::new(NopCertVerifier));
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(&cert))
            .rustls_client_config(config);
        let endpoint = Endpoint::from_static("https://api2.be.cash:8445").tls_config(tls_config)?;
        let bchd = BchrpcClient::connect(endpoint).await?;
        Ok(IndexerProduction { bchd, db, max_fetch_ahead: MAX_FETCH_AHEAD })
    }

    fn db(&self) -> &IndexDb {
        &self.db
    }

    async fn block_txs(&self, block_hash: &[u8]) -> Result<Vec<([u8; 32], TxMeta)>> {
        use bchrpc::{GetBlockRequest, get_block_request::HashOrHeight, block::transaction_data::TxidsOrTxs};
        let mut bchd = self.bchd.clone();
        let block = bchd.get_block(GetBlockRequest {
            full_transactions: false,
            hash_or_height: Some(HashOrHeight::Hash(block_hash.to_vec()))
        }).await?;
        let block = block.get_ref().block.as_ref().ok_or_else(|| anyhow!("Block not found"))?;
        let txs = block.transaction_data.iter().map(|tx_data| -> Result<_> {
            match &tx_data.txids_or_txs {
                Some(TxidsOrTxs::TransactionHash(tx_hash)) => {
                    let tx_hash: [u8; 32] = tx_hash.as_slice().try_into()?;
                    let tx_meta = self.db().tx_meta(&tx_hash)?.ok_or_else(|| anyhow!("Unindexed txs"))?;
                    Ok((tx_hash, tx_meta))
                }
                _ => bail!("Invalid tx hash"),
            }
        }).collect::<Result<Vec<_>, _>>()?;
        Ok(txs)
    }

    async fn tx(&self, tx_hash: &[u8]) -> Result<Tx> {
        use bchrpc::{GetTransactionRequest, GetRawTransactionRequest};
        let mut bchd1 = self.bchd.clone();
        let mut bchd2 = self.bchd.clone();
        let (tx, raw_tx) = tokio::try_join!(
            bchd1.get_transaction(GetTransactionRequest {
                hash: tx_hash.to_vec(),
                include_token_metadata: false,
            }),
            bchd2.get_raw_transaction(GetRawTransactionRequest {
                hash: tx_hash.to_vec(),
            }),
        )?;
        let tx = tx.get_ref();
        let tx = tx.transaction.as_ref().ok_or_else(|| anyhow!("No tx found"))?;
        let raw_tx = raw_tx.get_ref();
        let tx_meta = self.db.tx_meta(tx_hash)?.ok_or_else(|| anyhow!("No tx meta for tx"))?;
        let tx_out_spends = self.db.tx_out_spends(tx_hash)?;
        let token_meta = match tx.slp_transaction_info.as_ref() {
            Some(slp_info) if !slp_info.token_id.is_empty() => {
                self.db.token_meta(&slp_info.token_id)?
            }
            _ => None,
        };
        Ok(Tx {
            transaction: tx.clone(),
            tx_meta,
            token_meta,
            raw_tx: raw_tx.transaction.clone(),
            tx_out_spends,
        })
    }

    async fn run_indexer(self: Arc<Self>) {
        match self.run_indexer_inner().await {
            Ok(()) => {},
            Err(err) => eprintln!("Index error: {}", err),
        }
    }

    async fn run_indexer_inner(self: Arc<Self>) -> Result<()> {
        let last_height = self.db.last_block_height().unwrap() as usize;
        let current_height_atomic = Arc::new(AtomicUsize::new(last_height));
        let num_threads = 50;
        let (send_batches, mut receive_batches) = mpsc::channel(num_threads * 2);
        let (watch_height_sender, watch_height_receiver) = watch::channel(last_height);
        let mut join_handles = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            let indexer = Arc::clone(&self);
            let current_height_atomic = Arc::clone(&current_height_atomic);
            let send_batches = send_batches.clone();
            let watch_height_receiver = watch_height_receiver.clone();
            let join_handle = tokio::spawn(async move {
                indexer.index_thread(current_height_atomic, send_batches, watch_height_receiver).await
            });
            join_handles.push(join_handle);
        }
        std::mem::drop(send_batches);
        let mut current_height = last_height;
        let mut block_shelf = HashMap::new();
        let mut last_update_time = Instant::now();
        let mut last_update_blocks = 0;
        while let Some(block_batches) = receive_batches.recv().await {
            block_shelf.insert(block_batches.block_height as usize, block_batches);
            while block_shelf.contains_key(&current_height) {
                let block_batches = block_shelf.remove(&current_height).unwrap();
                self.db.apply_block_batches(block_batches)?;
                last_update_blocks += 1;
                let elapsed = last_update_time.elapsed().as_millis();
                if elapsed > 10_000 {
                    println!(
                        "Added {} blocks in {:.1}s, to block height {}",
                        last_update_blocks, elapsed as f64 / 1000.0, current_height,
                    );
                    println!("{} in shelf", block_shelf.len());
                    let flush_start = Instant::now();
                    self.db.flush()?;
                    println!("Flush took {:.2}s", flush_start.elapsed().as_secs_f64());
                    last_update_blocks = 0;
                    last_update_time = Instant::now();
                }
                current_height += 1;
                watch_height_sender.broadcast(current_height)?;
            }
        }
        for handle in join_handles {
            handle.await??;
        }
        self.update_mempool().await?;
        tokio::spawn({
            let indexer = Arc::clone(&self);
            async move { indexer.monitor_new_blocks().await }
        });
        tokio::spawn({
            let indexer = Arc::clone(&self);
            async move { indexer.monitor_mempool().await }
        });
        Ok(())
    }

    async fn index_thread(
        &self,
        current_height_atomic: Arc<AtomicUsize>,
        mut send_batches: mpsc::Sender<BlockBatches>,
        mut watch_height_receiver: watch::Receiver<usize>,
    ) -> Result<()> {
        use bchrpc::{GetBlockRequest, get_block_request::HashOrHeight};
        let mut bchd = self.bchd.clone();
        loop {
            let block_height = current_height_atomic.fetch_add(1, Ordering::SeqCst);
            while *watch_height_receiver.borrow() + self.max_fetch_ahead < block_height {
                println!("Waiting for BCHD to catch up, fetching block {} but processed only up to {}", block_height, *watch_height_receiver.borrow());
                watch_height_receiver.recv().await;
            }
            let result = bchd.get_block(GetBlockRequest {
                full_transactions: true,
                hash_or_height: Some(HashOrHeight::Height(block_height as i32)),
            }).await;
            match result {
                Ok(block) => {
                    if let Some(block) = &block.get_ref().block {
                        let batches = match self.db.make_block_batches(block) {
                            Ok(batches) => batches,
                            Err(err) => {
                                println!("make_block_batches (height {}): {:?}", block_height, err);
                                return Err(err);
                            },
                        };
                        let _ = send_batches.send(batches).await.map_err(|_| println!("Send failed"));
                    }
                }
                Err(err) if err.message() == "block not found" => {
                    return Ok(());
                }
                Err(err) => {
                    println!("Error message ({}): {}", block_height, err.message());
                    println!("Error detail ({}): {}", block_height, String::from_utf8_lossy(&err.details()));
                    return Err(err.into());
                }
            }
        }
    }

    async fn monitor_new_blocks(&self) {
        println!("Monitoring for new blocks");
        loop {
            match self.try_monitor_new_blocks().await {
                Ok(()) => println!("Block stream ended, restarting."),
                Err(err) => {
                    println!("Monitor blocks error: {:?}", err);
                    println!("Restarting monitor_blocks");
                }
            }
        }
    }

    async fn try_monitor_new_blocks(&self) -> Result<()> {
        use bchrpc::block_notification::Block;
        use bchrpc::SubscribeBlocksRequest;
        let mut bchd = self.bchd.clone();
        let mut block_stream = bchd
            .subscribe_blocks(SubscribeBlocksRequest {
                full_block: true,
                full_transactions: true,
                serialize_block: false,
            })
            .await?;
        while let Some(notification) = block_stream.get_mut().message().await? {
            if let Some(Block::MarshaledBlock(block)) = notification.block {
                println!("New block: {}", to_le_hex(&block.info.as_ref().unwrap().hash));
                let batches = self.db.make_block_batches(&block)?;
                self.db.apply_block_batches(batches)?;
                self.update_mempool().await?;
            }
        }
        Ok(())
    }

    async fn monitor_mempool(&self) {
        loop {
            match self.try_monitor_mempool().await {
                Ok(()) => println!("Block stream ended, restarting."),
                Err(err) => {
                    println!("Monitor post office error: {:?}", err);
                    println!("Restarting monitor_post_office");
                }
            }
        }
    }

    async fn try_monitor_mempool(&self) -> Result<()> {
        use bchrpc::{SubscribeTransactionsRequest, TransactionFilter, transaction_notification::Transaction};
        let mut bchd = self.bchd.clone();
        let mut tx_stream = bchd
            .subscribe_transactions(SubscribeTransactionsRequest {
                subscribe: Some(TransactionFilter {
                    all_transactions: true,
                    ..TransactionFilter::default()
                }),
                unsubscribe: None,
                include_mempool: true,
                include_in_block: false,
                serialize_tx: false,
            })
            .await?;
        while let Some(tx) = tx_stream.get_mut().message().await? {
            if let Some(Transaction::UnconfirmedTransaction(tx)) = tx.transaction {
                let tx = tx.transaction;
                if let Some(tx) = &tx {
                    let batch = self.db.make_mempool_tx_batches(&[&tx])?;
                    self.db.apply_batch(batch)?;
                    println!("Added tx {} to the mempool.", to_le_hex(&tx.hash));
                }
            }
        }
        Ok(())
    }

    async fn update_mempool(&self) -> Result<()> {
        use bchrpc::GetMempoolRequest;
        println!("Updating mempool...");
        let mut bchd = self.bchd.clone();
        let mempool = bchd.get_mempool(GetMempoolRequest {
            full_transactions: true,
        }).await?;
        let mempool = mempool.get_ref();
        let txs = self.db.make_mempool_txs(&mempool.transaction_data)?;
        let batch = self.db.make_mempool_tx_batches(&txs)?;
        self.db.clear_mempool()?;
        self.db.apply_batch(batch)?;
        println!("Added {} txs to the mempool", txs.len());
        Ok(())
    }
}

#[async_trait]
impl Indexer for IndexerDevelopment {
    async fn connect(db: IndexDb) -> Result<Self> {
        const MAX_FETCH_AHEAD: usize = 1000;
        Ok(IndexerDevelopment {
            db,
            max_fetch_ahead: MAX_FETCH_AHEAD,
            scheduled_for_termination: Arc::new(AtomicBool::new(false))
        })
    }

    fn db(&self) -> &IndexDb {
        &self.db
    }

    async fn block_txs(&self, block_hash: &[u8]) -> Result<Vec<([u8; 32], TxMeta)>> {
        let block_meta = self.db().block_meta(block_hash)?.ok_or_else(|| anyhow!("No block meta for hash"))?;
        return self.db().block_height_txs(block_meta.height);
    }

    async fn tx(&self, tx_hash: &[u8]) -> Result<Tx> {
        // NOTE: On the production implementation the transaction is fetched directly
        // from the network, in development we don't have that luxury, so for now
        // we're just feeding random data along with what we can query from the db (the meta)
        let tx_meta = self.db.tx_meta(tx_hash)?.ok_or_else(|| anyhow!("No tx meta"))?;
        let tx_out_spends = self.db.tx_out_spends(tx_hash)?;

        let tx = mocker::generate_transaction(0, &vec![0, 0, 0])?;

        let token_meta = match tx_meta.variant {
            TxMetaVariant::Slp{token_id, ..} => {
                println!("fn tx: meta matched");

                let stuff = self.db.token_meta(&token_id)?;
                println!("{:?}", stuff);

                stuff
            }
            _ => None,
        };
        Ok(Tx {
            transaction: tx.clone(),
            tx_meta,
            token_meta,
            raw_tx: vec![],
            tx_out_spends,
        })
    }

    async fn run_indexer(self: Arc<Self>) {
        let scheduled_for_termination = self.scheduled_for_termination.clone();

        ctrlc::set_handler(move || {
            if scheduled_for_termination.load(Ordering::Relaxed) {
                println!("\nExiting...");
                std::process::exit(0);
            } else {
                println!("\nShutting down indexer, the web server will continue to operate (send another signal to terminate)!");
                scheduled_for_termination.store(true, Ordering::Relaxed);
            }
        })
        .expect("Error setting Ctrl-C handler");

        match self.run_indexer_inner().await {
            Ok(()) => {},
            Err(err) => eprintln!("Index error: {}", err),
        }
    }

    async fn run_indexer_inner(self: Arc<Self>) -> Result<()> {
        println!("Starting up mock indexer");
        let last_height = self.db.last_block_height().unwrap() as usize;
        let current_height_atomic = Arc::new(AtomicUsize::new(last_height));
        let num_threads = 1;
        let (send_batches, mut receive_batches) = mpsc::channel(num_threads * 2);
        let (watch_height_sender, watch_height_receiver) = watch::channel(last_height);
        let mut join_handles = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            let indexer = Arc::clone(&self);
            let current_height_atomic = Arc::clone(&current_height_atomic);
            let send_batches = send_batches.clone();
            let watch_height_receiver = watch_height_receiver.clone();
            let join_handle = tokio::spawn(async move {
                indexer.index_thread(current_height_atomic, send_batches, watch_height_receiver).await
            });
            join_handles.push(join_handle);
        }
        std::mem::drop(send_batches);
        let mut current_height = last_height;
        let mut block_shelf = HashMap::new();
        let mut last_update_time = Instant::now();
        let mut last_update_blocks = 0;
        while let Some(block_batches) = receive_batches.recv().await {
            block_shelf.insert(block_batches.block_height as usize, block_batches);
            while block_shelf.contains_key(&current_height) {
                let block_batches = block_shelf.remove(&current_height).unwrap();
                self.db.apply_block_batches(block_batches)?;
                last_update_blocks += 1;
                let elapsed = last_update_time.elapsed().as_millis();
                if elapsed > 10_000 {
                    println!(
                        "Added {} blocks in {:.1}s, to block height {}",
                        last_update_blocks, elapsed as f64 / 1000.0, current_height,
                    );
                    println!("{} in shelf", block_shelf.len());
                    let flush_start = Instant::now();
                    self.db.flush()?;
                    println!("Flush took {:.2}s", flush_start.elapsed().as_secs_f64());
                    last_update_blocks = 0;
                    last_update_time = Instant::now();
                }
                current_height += 1;
                watch_height_sender.broadcast(current_height)?;
            }
        }
        for handle in join_handles {
            handle.await??;
        }
        self.update_mempool().await?;
        tokio::spawn({
            let indexer = Arc::clone(&self);
            async move { indexer.monitor_new_blocks().await }
        });
        tokio::spawn({
            let indexer = Arc::clone(&self);
            async move { indexer.monitor_mempool().await }
        });

        Ok(())
    }

    async fn index_thread(
        &self,
        current_height_atomic: Arc<AtomicUsize>,
        mut send_batches: mpsc::Sender<BlockBatches>,
        mut watch_height_receiver: watch::Receiver<usize>,
    ) -> Result<()> {
        loop {
            let block_height = current_height_atomic.fetch_add(1, Ordering::SeqCst);
            while *watch_height_receiver.borrow() + self.max_fetch_ahead < block_height {
                println!("Waiting for BCHD to catch up, fetching block {} but processed only up to {}", block_height, *watch_height_receiver.borrow());
                watch_height_receiver.recv().await;
            }
            let block_hash = mocker::generate_random_sha256()?;
            let transaction = mocker::generate_transaction(block_height as i32, &block_hash)?;
            let transaction_data = mocker::generate_transaction_data(transaction);
            let block = mocker::generate_block(block_height as i32, &block_hash, vec![transaction_data])?;

            let batches = match self.db.make_block_batches(&block) {
                Ok(batches) => batches,
                Err(err) => {
                    println!("make_block_batches (height {}): {:?}", block_height, err);
                    return Err(err);
                },
            };
            let _ = send_batches.send(batches).await.map_err(|_| println!("Send failed"));

            if self.scheduled_for_termination.load(Ordering::Relaxed) {
                break Ok(());
            }
        }
    }

    async fn monitor_new_blocks(&self) {
    }

    async fn try_monitor_new_blocks(&self) -> Result<()> {
        Ok(())
    }

    async fn monitor_mempool(&self) {
    }

    async fn try_monitor_mempool(&self) -> Result<()> {
        Ok(())
    }

    async fn update_mempool(&self) -> Result<()> {
        Ok(())
    }
}

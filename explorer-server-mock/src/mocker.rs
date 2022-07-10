use bitcoinsuite_chronik_client::proto::{OutPoint, ScriptUtxos, SlpMeta, SlpToken, Utxo};
use rand::{prelude::ThreadRng, Rng};

use explorer_server::blockchain::to_be_hex;

pub struct Mocker {
    rand: ThreadRng,
}

impl Mocker {
    pub fn setup() -> Mocker {
        let rand = rand::thread_rng();
        Mocker { rand }
    }
}

impl Mocker {
    pub fn create_slp(&mut self, amount: u64) -> (SlpToken, SlpMeta) {
        (
            SlpToken {
                amount,
                is_mint_baton: false,
            },
            SlpMeta {
                token_type: 0,
                tx_type: 0,
                token_id: (0..24).map(|_| self.rand.gen_range(0..255)).collect(),
                group_token_id: (0..24).map(|_| self.rand.gen_range(0..255)).collect(),
            },
        )
    }

    pub fn create_cash_utxo(&mut self, value: i64) -> (String, Utxo) {
        let txid: Vec<u8> = (0..24).map(|_| self.rand.gen_range(0..255)).collect();
        (
            to_be_hex(&txid),
            Utxo {
                outpoint: Some(OutPoint { txid, out_idx: 0 }),
                block_height: self.rand.gen_range(0..999999),
                is_coinbase: false,
                value,
                slp_meta: None,
                slp_token: None,
                network: 2,
            },
        )
    }

    pub fn create_token_utxo(
        &mut self,
        value: i64,
        slp_token: SlpToken,
        slp_meta: SlpMeta,
    ) -> (String, Utxo) {
        let txid: Vec<u8> = (0..24).map(|_| self.rand.gen_range(0..255)).collect();
        (
            to_be_hex(&txid),
            Utxo {
                outpoint: Some(OutPoint { txid, out_idx: 0 }),
                block_height: self.rand.gen_range(0..999999),
                is_coinbase: false,
                value,
                slp_token: Some(slp_token),
                slp_meta: Some(slp_meta),
                network: 2,
            },
        )
    }

    pub fn create_script_utxos(&mut self, utxos: Vec<Utxo>) -> ScriptUtxos {
        ScriptUtxos {
            output_script: (0..24).map(|_| self.rand.gen_range(0..255)).collect(),
            utxos,
        }
    }
}

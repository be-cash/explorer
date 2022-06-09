use bitcoinsuite_chronik_client::proto::Utxo;
use bitcoinsuite_chronik_client::{proto::Utxos, ChronikClient};
use bitcoinsuite_core::CashAddress;
use bitcoinsuite_error::Result;
use explorer_server::blockchain::to_be_hex;
use explorer_server_mock::mocker::Mocker;
use httpmock::prelude::*;
use json::object;
use prost::Message;

use explorer_server::{blockchain::cash_addr_to_script_type_payload, server::Server};

#[tokio::test]
async fn data_address_main_balance_exists() -> Result<()> {
    let address_hex = "ecash:qp3wjpa3tjlj042z2wv7hahsldgwhwy0rquas9fmzn";
    let address = CashAddress::parse_cow(address_hex.into())?;
    let (_, script_payload) = cash_addr_to_script_type_payload(&address);

    let mut mocker = Mocker::setup();
    let (txid, utxo) = mocker.create_cash_utxo(123);
    let script_utxos = mocker.create_script_utxos(vec![utxo.clone()]);

    let utxos = Utxos {
        script_utxos: vec![script_utxos],
    };
    let buf = utxos.encode_to_vec();

    let chronik_server = MockServer::start();
    chronik_server.mock(|when, then| {
        when.path(&format!(
            "/xec/script/p2pkh/{}/utxos",
            hex::encode(script_payload)
        ));
        then.status(200).body(buf.as_slice());
    });

    let server_url =
        explorer_server_mock::server::setup_and_run(chronik_server.url("/xec")).await?;

    let response = reqwest::get(&format!(
        "http://{}/api/address/{}/balances",
        server_url, address_hex
    ))
    .await?
    .text()
    .await?;

    let response = json::parse(&response)?;

    assert_eq!(
        response,
        object! {
            data: {
                main: {
                    tokenId: json::JsonValue::Null,
                    satsAmount: utxo.value,
                    tokenAmount: 0,
                    utxos: [
                        {
                            txHash: txid.to_string(),
                            outIdx: utxo.outpoint.clone().expect("Impossible").out_idx,
                            satsAmount: utxo.value,
                            tokenAmount: 0,
                            isCoinbase: utxo.is_coinbase,
                            blockHeight: utxo.block_height,
                        }
                    ]
                }
            }
        }
    );

    Ok(())
}

#[tokio::test]
async fn data_address_sats_amount() -> Result<()> {
    let address_hex = "ecash:qp3wjpa3tjlj042z2wv7hahsldgwhwy0rquas9fmzn";
    let address = CashAddress::parse_cow(address_hex.into())?;
    let (_, script_payload) = cash_addr_to_script_type_payload(&address);

    let mut mocker = Mocker::setup();
    let utxos: Vec<Utxo> = [100, 100]
        .map(|amount| {
            let (_, utxo) = mocker.create_cash_utxo(amount);
            utxo
        })
        .into();
    let script_utxos = mocker.create_script_utxos(utxos);

    let utxos = Utxos {
        script_utxos: vec![script_utxos],
    };
    let buf = utxos.encode_to_vec();

    let chronik_server = MockServer::start();
    chronik_server.mock(|when, then| {
        when.path(&format!(
            "/xec/script/p2pkh/{}/utxos",
            hex::encode(script_payload)
        ));
        then.status(200).body(buf.as_slice());
    });

    let chronik = ChronikClient::new(chronik_server.url("/xec")).expect("Impossible");
    let server = Server::setup(chronik).await?;

    let response = server.data_address_balances(address_hex).await?;

    assert_eq!(response.data["main"].sats_amount, 200);

    Ok(())
}

#[tokio::test]
async fn data_address_token_balance_exists() -> Result<()> {
    let address_hex = "ecash:qp3wjpa3tjlj042z2wv7hahsldgwhwy0rquas9fmzn";
    let address = CashAddress::parse_cow(address_hex.into())?;
    let (_, script_payload) = cash_addr_to_script_type_payload(&address);

    let mut mocker = Mocker::setup();
    let (slp_token, slp_meta) = mocker.create_slp(10);
    let token_id = to_be_hex(&slp_meta.token_id);

    let (_, utxo) = mocker.create_token_utxo(123, slp_token, slp_meta);
    let script_utxos = mocker.create_script_utxos(vec![utxo.clone()]);

    let utxos = Utxos {
        script_utxos: vec![script_utxos],
    };
    let buf = utxos.encode_to_vec();

    let chronik_server = MockServer::start();
    chronik_server.mock(|when, then| {
        when.path(&format!(
            "/xec/script/p2pkh/{}/utxos",
            hex::encode(script_payload)
        ));
        then.status(200).body(buf.as_slice());
    });

    let chronik = ChronikClient::new(chronik_server.url("/xec")).expect("Impossible");
    let server = Server::setup(chronik).await?;

    let response = server.data_address_balances(address_hex).await?;

    assert_eq!(response.data.keys().len(), 2);
    assert!(response.data.contains_key(&token_id));
    assert_eq!(response.data[&token_id].utxos.len(), 1);

    Ok(())
}

#[tokio::test]
async fn data_address_token_amount() -> Result<()> {
    let address_hex = "ecash:qp3wjpa3tjlj042z2wv7hahsldgwhwy0rquas9fmzn";
    let address = CashAddress::parse_cow(address_hex.into())?;
    let (_, script_payload) = cash_addr_to_script_type_payload(&address);

    let mut mocker = Mocker::setup();
    let (slp_token, slp_meta) = mocker.create_slp(100);
    let token_id = to_be_hex(&slp_meta.token_id);

    let (txid, utxo) = mocker.create_token_utxo(123, slp_token, slp_meta);
    let script_utxos = mocker.create_script_utxos(vec![utxo.clone()]);

    let utxos = Utxos {
        script_utxos: vec![script_utxos],
    };
    let buf = utxos.encode_to_vec();

    let chronik_server = MockServer::start();
    chronik_server.mock(|when, then| {
        when.path(&format!(
            "/xec/script/p2pkh/{}/utxos",
            hex::encode(script_payload)
        ));
        then.status(200).body(buf.as_slice());
    });

    let server_url =
        explorer_server_mock::server::setup_and_run(chronik_server.url("/xec")).await?;

    let response = reqwest::get(&format!(
        "http://{}/api/address/{}/balances",
        server_url, address_hex
    ))
    .await?
    .text()
    .await?;

    let response = json::parse(&response)?;

    assert_eq!(
        response,
        object! {
            data: {
                main: {
                    tokenId: json::JsonValue::Null,
                    satsAmount: 0,
                    tokenAmount: 0,
                    utxos: [ ]
                },
                [&token_id]: {
                     tokenId: token_id.to_string(),
                     satsAmount: utxo.value,
                     tokenAmount: utxo.slp_token.clone().expect("Impossible").amount,
                     utxos: [
                        {
                           txHash: txid.to_string(),
                           outIdx: utxo.outpoint.clone().expect("Impossible").out_idx,
                           satsAmount: utxo.value,
                           tokenAmount: utxo.slp_token.clone().expect("Impossible").amount,
                           isCoinbase: utxo.is_coinbase,
                           blockHeight: utxo.block_height,
                        }
                     ]
                }
            }
        }
    );

    Ok(())
}

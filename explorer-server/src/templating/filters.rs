use chrono_humanize::HumanTime;
use maud::{html, PreEscaped};
use zerocopy::AsBytes;
use chrono::DateTime;

use humansize::{FileSize, file_size_opts as options};
use num_format::{Locale, ToFormattedString};
use bitcoin_cash::Script;

use crate::{
    grpc::bchrpc::{SlpToken, transaction::input::Outpoint},
    blockchain,
    primitives::SlpAction,
};

fn render_integer_with_small_flag(int: &u64, smallify: bool) -> askama::Result<String> {
    let string = int.to_formatted_string(&Locale::en);
    let parts = string.split(",").collect::<Vec<_>>();
    let output = html! {
        @for (idx, part) in parts.iter().enumerate() {
            @if idx >= 2 && smallify {
                small.digit-sep[idx < parts.len() - 1] { (part) }
            } @else {
                span.digit-sep[idx < parts.len() - 1] { (part) }
            }
        }
    };

    Ok(output.into_string())
}

pub fn max<'a>(value: &'a i64, maximum: &'a i64) -> askama::Result<i64> {
    Ok(*value.max(maximum))
}

pub fn get_token_type<'a>(token_type: &'a u32) -> askama::Result<String> {
    let type_string = match token_type {
        0x01 => "Type1",
        0x41 => "NFT1 Child",
        0x81 => "NFT1 Group",
        _ => "",
    };

    Ok(type_string.into())
}

pub fn get_action_string<'a>(action: &'a SlpAction) -> askama::Result<String> {
  let action_str = match action {
      SlpAction::SlpV1Genesis => "GENESIS",
      SlpAction::SlpV1Mint => "MINT",
      SlpAction::SlpV1Send => "SEND",
      SlpAction::SlpV1Nft1GroupGenesis => "NFT1 Group GENESIS",
      SlpAction::SlpV1Nft1GroupMint => "NFT1 MINT",
      SlpAction::SlpV1Nft1GroupSend => "NFT1 Group SEND",
      SlpAction::SlpV1Nft1UniqueChildGenesis => "NFT1 Child GENESIS",
      SlpAction::SlpV1Nft1UniqueChildSend => "NFT1 Child SEND",
  };

  Ok(action_str.into())
}

pub fn check_is_coinbase<'a>(outpoint: &'a Outpoint) -> askama::Result<bool> {
    Ok(blockchain::is_coinbase(outpoint))
}

pub fn destination_from_script<'a>(script: &'a Vec<u8>, is_token: &bool) -> askama::Result<blockchain::Destination<'a>> {
    let prefix = if *is_token { "etoken" } else { "ecash" };
    Ok(blockchain::destination_from_script(prefix, script))
}

pub fn get_script<'a>(signature_script: &'a Vec<u8>) -> askama::Result<String> {
    let script = Script::deser_ops(signature_script.as_slice().into())
            .map(|script| script.to_string())
            .unwrap_or("invalid script".to_string());
    Ok(script)
}

pub fn check_is_token(slp_token: &Option<SlpToken>) -> askama::Result<bool> {
    Ok(slp_token.as_ref().map(|slp| slp.amount > 0 || slp.is_mint_baton).unwrap_or(false))
}

pub fn human_time<'a>(timestamp: &'a DateTime<chrono::Utc>) -> askama::Result<HumanTime> {
    Ok(HumanTime::from(*timestamp))
}

pub fn render_integer(int: &u64) -> askama::Result<String> {
    render_integer_with_small_flag(int, false)
}

pub fn render_integer_smallify(int: &u64) -> askama::Result<String> {
    render_integer_with_small_flag(int, true)
}

pub fn render_human_size<'a>(value: &'a u64) -> askama::Result<String> {
    Ok(value.file_size(options::CONVENTIONAL).unwrap())
}

pub fn _old_render_byte_size(size: &u64, is_long: &bool) -> askama::Result<String> {
    let output;

    let bytes = html! {
        @if *is_long {
            small {
                " ("
                ((render_integer(size)?)) // NOTE: this is probably problematic! does maud escape this?
                " B)"
            }
        }
    };

    if *size < 1024 {
        output = html! { (size) " B" };
    } else if *size < 1024 * 1024 {
        output = html! {
            (format!("{:.2}", *size as f64 / 1000.0))
            " kB"
            (bytes)
        };
    } else {
        output = html! {
            (format!("{:.2}", *size as f64 / 1000_0000.0))
            " MB"
            (bytes)
        };
    }

    Ok(output.into_string())
}

pub fn render_difficulty(difficulty: &f64) -> askama::Result<String> {
    let est_hashrate = difficulty * (0xffffffffu64 as f64) / 600.0;
    let hashrate= if est_hashrate < 1e12 {
        html! { (format!("{:.2} GH/s", est_hashrate / 1e9)) }
    } else if est_hashrate < 1e15 {
        html! { (format!("{:.2} TH/s", est_hashrate / 1e12)) }
    } else if est_hashrate < 1e18 {
        html! { (format!("{:.2} PH/s", est_hashrate / 1e15)) }
    } else {
        html! { (format!("{:.2} EH/s", est_hashrate / 1e18)) }
    };
    let num_digits = difficulty.log10().floor();
    let exponent = (num_digits / 3.0) as u32;
    let difficulty = match exponent {
        0 => html! { (format!("{:.0}", difficulty)) },
        1 => html! { (format!("{:.2}", difficulty / 1e3)) " ×10" sup { "3" } },
        2 => html! { (format!("{:.2}", difficulty / 1e6)) " ×10" sup { "6" } },
        3 => html! { (format!("{:.2}", difficulty / 1e9)) " ×10" sup { "9" } },
        4 => html! { (format!("{:.2}", difficulty / 1e12)) " ×10" sup { "12" } },
        _ => html! { (format!("{:.2}", difficulty / 1e15)) " ×10" sup { "15" } },
    };

    let output = html! {
        (difficulty)
        small {
            " (10 min. blocks = "
            (hashrate)
            ")"
        }
    };
    Ok(output.into_string())
}

pub fn render_integer_with_commas(int: &u64) -> askama::Result<String> {
    let string = int.to_formatted_string(&Locale::en);
    let parts = string.split(",").collect::<Vec<_>>();

    let output = html! {
        @for (idx, part) in parts.iter().enumerate() {
            @if idx != 0 {
                span.non-selectable { "," }
            }
            layflags-rolling-number { (part) }
        }
    };

    Ok(output.into_string())
}

pub fn render_sats(sats: &i64) -> askama::Result<String> {
    let coins = *sats as f64 / 100.0;
    let fmt = format!("{:.2}", coins);
    let mut parts = fmt.split(".");
    let integer_part: u64 = parts.next().unwrap().parse().unwrap();
    let fract_part = parts.next().unwrap();
    let output;

    if fract_part == "00" {
        output = render_integer_with_commas(&integer_part)?;
    } else {
        let _output = html! {
            (PreEscaped(render_integer_with_commas(&integer_part)?))
            "."
            span.small {
                layflags-rolling-number {
                    (fract_part)
                }
            }
        };
        output = _output.into_string();
    }

    Ok(output)
}

pub fn hexify_block_header<'a>(block_header: &'a blockchain::BlockHeader) -> askama::Result<String> {
    Ok(hex::encode(block_header.as_bytes()))
}

pub fn hexify_u8_vector_bytes<'a>(value: &'a Vec<u8>) -> askama::Result<String> {
    Ok(hex::encode(value.as_bytes()))
}

pub fn hexify_u8_vector<'a>(value: &'a Vec<u8>) -> askama::Result<String> {
    Ok(hex::encode(value))
}

pub fn string_from_lossy_utf8<'a>(value: &'a Vec<u8>) -> askama::Result<String> {
    Ok(String::from_utf8_lossy(value).to_string())
}

pub fn to_le_hex(slice: &[u8]) -> askama::Result<String> {
    Ok(blockchain::to_le_hex(slice))
}

pub fn u32_to_u64(value: &u32) -> askama::Result<u64> {
    Ok(*value as u64)
}

pub fn i64_to_u64(value: &i64) -> askama::Result<u64> {
    Ok(*value as u64)
}

pub fn i32_to_u64(value: &i32) -> askama::Result<u64> {
    Ok(*value as u64)
}

pub fn render_token_amount(base_amount: &u64, decimals: &u32) -> askama::Result<String> {
    let decimals = *decimals as usize;
    if decimals == 0 {
        return render_integer(base_amount);
    }
    let base_amount_str = format!("{:0digits$}", base_amount, digits = decimals + 1);
    let decimal_idx = base_amount_str.len() - decimals;
    let integer_part: u64 = base_amount_str[..decimal_idx].parse().unwrap();
    let fract_part = &base_amount_str[decimal_idx..];
    let num_fract_sections = (decimals as usize + 2) / 3;
    let mut all_zeros = true;
    let mut rendered = html!{};
    for section_idx in (0..num_fract_sections).rev() {
        let offset = section_idx * 3;
        let section = &fract_part[offset..fract_part.len().min(offset+3)];
        if !section.chars().all(|c| c == '0') {
            all_zeros = false;
        }
        rendered = html! {
            small.zeros[all_zeros].digit-sep[section_idx != num_fract_sections - 1] {
                (section)
            }
            (rendered)
        };
    }
    let output = html! { (PreEscaped(render_integer(&integer_part)?)) "." (rendered) };
    Ok(output.into_string())
}

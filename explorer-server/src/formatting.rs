use maud::{Markup, html};
use num_format::{Locale, ToFormattedString};

pub fn render_byte_size(size: u64, is_long: bool) -> Markup {
    let bytes = html! {
        @if is_long {
            small {
                " ("
                ((render_integer(size)))
                " B)"
            }
        }
    };
    if size < 1024 {
        return html! { (size) " B" }
    } else if size < 1024 * 1024 {
        return html! {
            (format!("{:.2}", size as f64 / 1000.0))
            " kB"
            (bytes)
        }
    } else {
        return html! {
            (format!("{:.2}", size as f64 / 1000_0000.0))
            " MB"
            (bytes)
        }
    }
}

pub fn render_integer_with_commas(int: u64) -> Markup {
    let string = int.to_formatted_string(&Locale::en);
    html! {
        span { (string) }
    }
}

pub fn render_integer(int: u64) -> Markup {
    let string = int.to_formatted_string(&Locale::en);
    let parts = string.split(",").collect::<Vec<_>>();
    html! {
        @for (idx, part) in parts.iter().enumerate() {
            @if idx >= 2 {
                small.digit-sep[idx < parts.len() - 1] { (part) }
            } @else {
                span.digit-sep[idx < parts.len() - 1] { (part) }
            }
        }
    }
}

pub fn render_difficulty(difficulty: f64) -> Markup {
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
    html! {
        (difficulty)
        small {
            " (10 min. blocks = "
            (hashrate)
            ")"
        }
    }
}

pub fn render_sats(sats: i64) -> Markup {
    let coins = sats as f64 / 100.0;
    let fmt = format!("{:.2}", coins);
    let mut parts = fmt.split(".");
    let integer_part: u64 = parts.next().unwrap().parse().unwrap();
    let fract_part = parts.next().unwrap();
    if fract_part == "00" {
        render_integer_with_commas(integer_part)
    } else {
        html! {
            (render_integer_with_commas(integer_part))
            "."
            small {
                (fract_part)
            }
        }
    }
}

pub fn render_amount(base_amount: u64, decimals: u32) -> Markup {
    let decimals = decimals as usize;
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
    html! { (render_integer(integer_part)) "." (rendered) }
}

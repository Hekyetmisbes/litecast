// High-Performance Unit, Timezone, Epoch & Currency Converters for LiteCast.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::get_app_storage_dir;
use crate::search::{ResultAction, SearchResult};

#[derive(Debug, Clone)]
pub struct ConverterResult {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub copy_value: String,
    pub badge: &'static str,
}

pub fn parse_converters(input: &str) -> Vec<ConverterResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();

    // 1. Unix Epoch / Time Converter
    if let Some(epoch_res) = parse_epoch_converter(trimmed) {
        results.push(epoch_res);
    }

    // 2. World Timezone Converter ("time in <city>")
    if let Some(tz_res) = parse_timezone_query(trimmed) {
        results.push(tz_res);
    }

    // 3. Unit Converter (Length, Mass, Temperature, Digital Data)
    if let Some(unit_res) = parse_unit_conversion(trimmed) {
        results.push(unit_res);
    }

    // 4. Currency & Crypto Converter
    if let Some(curr_res) = parse_currency_conversion(trimmed) {
        results.push(curr_res);
    }

    results
}

// -----------------------------------------------------------------------------
// 1. Unix Epoch Converter
// -----------------------------------------------------------------------------
fn parse_epoch_converter(input: &str) -> Option<ConverterResult> {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();

    if trimmed == "epoch" || trimmed == "unix" || trimmed == "timestamp" || trimmed == "now" {
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        return Some(ConverterResult {
            id: "conv_epoch_now".to_string(),
            title: format!("⏱️ Unix Zaman Damgası: {}", now_sec),
            preview: format!("Milisaniye: {}  •  Enter ile kopyalayın", now_ms),
            copy_value: format!("{}", now_sec),
            badge: "Zaman",
        });
    }

    let val_str = if let Some(rest) = trimmed.strip_prefix("unix ") {
        rest.trim()
    } else if let Some(rest) = trimmed.strip_prefix("epoch ") {
        rest.trim()
    } else {
        let rest = trimmed.strip_prefix("timestamp ")?;
        rest.trim()
    };

    let ts: i64 = val_str.parse().ok()?;
    let (sec, _is_ms) = if ts > 100_000_000_000 {
        (ts / 1000, true)
    } else {
        (ts, false)
    };

    let formatted_utc = format_epoch_utc(sec);

    Some(ConverterResult {
        id: format!("conv_epoch_{}", ts),
        title: format!("📅 Zaman: {}", formatted_utc),
        preview: format!("Girdi: {} (Unix Epoch)  •  Enter ile kopyalayın", ts),
        copy_value: formatted_utc,
        badge: "Zaman Damgası",
    })
}

fn format_epoch_utc(epoch_sec: i64) -> String {
    let mut total_days = epoch_sec / 86400;
    let mut remaining_sec = epoch_sec % 86400;
    if remaining_sec < 0 {
        remaining_sec += 86400;
        total_days -= 1;
    }

    let hours = remaining_sec / 3600;
    let minutes = (remaining_sec % 3600) / 60;
    let seconds = remaining_sec % 60;

    // Civil day algorithm
    let z = total_days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y_final = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        y_final, m, d, hours, minutes, seconds
    )
}

// -----------------------------------------------------------------------------
// 2. World Timezone Converter
// -----------------------------------------------------------------------------
fn parse_timezone_query(input: &str) -> Option<ConverterResult> {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();

    let target = if let Some(rest) = trimmed.strip_prefix("time in ") {
        rest.trim()
    } else if let Some(rest) = trimmed.strip_prefix("saat ") {
        rest.trim()
    } else {
        let rest = trimmed.strip_prefix("time ")?;
        rest.trim()
    };

    let (city_name, offset_hours): (&'static str, f64) = match target {
        "tokyo" | "japan" | "japonya" => ("Tokyo (Japonya)", 9.0),
        "london" | "londra" | "uk" | "ingiltere" => ("Londra (İngiltere)", 0.0),
        "new york" | "ny" | "nyc" => ("New York (ABD)", -5.0),
        "los angeles" | "la" | "california" => ("Los Angeles (ABD)", -8.0),
        "san francisco" | "sf" => ("San Francisco (ABD)", -8.0),
        "berlin" | "germany" | "almanya" => ("Berlin (Almanya)", 1.0),
        "paris" | "france" | "fransa" => ("Paris (Fransa)", 1.0),
        "istanbul" | "turkey" | "türkiye" => ("İstanbul (Türkiye)", 3.0),
        "dubai" | "uae" => ("Dubai (BAE)", 4.0),
        "singapore" | "singapur" => ("Singapur", 8.0),
        "hong kong" | "hk" => ("Hong Kong", 8.0),
        "sydney" | "australia" | "avustralya" => ("Sidney (Avustralya)", 11.0),
        "utc" | "gmt" => ("UTC / GMT", 0.0),
        _ => return None,
    };

    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let target_sec = now_sec + (offset_hours * 3600.0) as i64;
    let formatted_time = format_epoch_utc(target_sec);
    let time_only = formatted_time
        .split_whitespace()
        .nth(1)
        .unwrap_or("00:00:00");

    Some(ConverterResult {
        id: format!("tz_{}", target),
        title: format!("🌍 {}: {}", city_name, time_only),
        preview: format!(
            "Tarih: {}  •  UTC Farkı: {:+}",
            formatted_time, offset_hours
        ),
        copy_value: format!("{} {}", city_name, time_only),
        badge: "Dünya Saati",
    })
}

// -----------------------------------------------------------------------------
// 3. Unit Converter
// -----------------------------------------------------------------------------
fn parse_unit_conversion(input: &str) -> Option<ConverterResult> {
    let lower = input.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();

    // Pattern: <val> <from_unit> to/in <to_unit>
    // e.g. "100 km to miles", "5 ft in cm", "16 gb to mb", "100 c to f"
    if parts.len() != 4 || (parts[2] != "to" && parts[2] != "in" && parts[2] != "=") {
        return None;
    }

    let val: f64 = parts[0].parse().ok()?;
    let from_unit = parts[1];
    let to_unit = parts[3];

    // Temperature
    if is_temp_unit(from_unit) && is_temp_unit(to_unit) {
        let res = convert_temperature(val, from_unit, to_unit)?;
        return Some(ConverterResult {
            id: format!("unit_temp_{}_{}", from_unit, to_unit),
            title: format!("🌡️ Sıcaklık: {:.2} {}", res, to_unit.to_uppercase()),
            preview: format!(
                "{:.2} {} = {:.2} {}",
                val,
                from_unit.to_uppercase(),
                res,
                to_unit.to_uppercase()
            ),
            copy_value: format!("{:.2}", res),
            badge: "Birim Çevirici",
        });
    }

    // Length
    if let Some(res) = convert_length(val, from_unit, to_unit) {
        return Some(ConverterResult {
            id: format!("unit_len_{}_{}", from_unit, to_unit),
            title: format!("📏 Uzunluk: {:.4} {}", res, to_unit),
            preview: format!("{} {} = {:.4} {}", val, from_unit, res, to_unit),
            copy_value: format_clean_f64(res),
            badge: "Birim Çevirici",
        });
    }

    // Mass / Weight
    if let Some(res) = convert_mass(val, from_unit, to_unit) {
        return Some(ConverterResult {
            id: format!("unit_mass_{}_{}", from_unit, to_unit),
            title: format!("⚖️ Ağırlık: {:.4} {}", res, to_unit),
            preview: format!("{} {} = {:.4} {}", val, from_unit, res, to_unit),
            copy_value: format_clean_f64(res),
            badge: "Birim Çevirici",
        });
    }

    // Digital Data Storage
    if let Some(res) = convert_data_storage(val, from_unit, to_unit) {
        return Some(ConverterResult {
            id: format!("unit_data_{}_{}", from_unit, to_unit),
            title: format!("💾 Veri Boyutu: {:.2} {}", res, to_unit.to_uppercase()),
            preview: format!(
                "{} {} = {:.2} {}",
                val,
                from_unit.to_uppercase(),
                res,
                to_unit.to_uppercase()
            ),
            copy_value: format_clean_f64(res),
            badge: "Birim Çevirici",
        });
    }

    None
}

fn is_temp_unit(u: &str) -> bool {
    matches!(u, "c" | "celsius" | "f" | "fahrenheit" | "k" | "kelvin")
}

fn convert_temperature(val: f64, from: &str, to: &str) -> Option<f64> {
    let c = match from {
        "c" | "celsius" => val,
        "f" | "fahrenheit" => (val - 32.0) * 5.0 / 9.0,
        "k" | "kelvin" => val - 273.15,
        _ => return None,
    };

    match to {
        "c" | "celsius" => Some(c),
        "f" | "fahrenheit" => Some(c * 9.0 / 5.0 + 32.0),
        "k" | "kelvin" => Some(c + 273.15),
        _ => None,
    }
}

fn convert_length(val: f64, from: &str, to: &str) -> Option<f64> {
    let meters = match from {
        "m" | "meter" | "meters" | "metre" => val,
        "km" | "kilometer" | "kilometers" => val * 1000.0,
        "cm" | "centimeter" | "centimeters" => val * 0.01,
        "mm" | "millimeter" | "millimeters" => val * 0.001,
        "mile" | "miles" | "mi" => val * 1609.344,
        "yard" | "yards" | "yd" => val * 0.9144,
        "ft" | "foot" | "feet" => val * 0.3048,
        "inch" | "inches" | "in" => val * 0.0254,
        _ => return None,
    };

    match to {
        "m" | "meter" | "meters" | "metre" => Some(meters),
        "km" | "kilometer" | "kilometers" => Some(meters / 1000.0),
        "cm" | "centimeter" | "centimeters" => Some(meters / 0.01),
        "mm" | "millimeter" | "millimeters" => Some(meters / 0.001),
        "mile" | "miles" | "mi" => Some(meters / 1609.344),
        "yard" | "yards" | "yd" => Some(meters / 0.9144),
        "ft" | "foot" | "feet" => Some(meters / 0.3048),
        "inch" | "inches" | "in" => Some(meters / 0.0254),
        _ => None,
    }
}

fn convert_mass(val: f64, from: &str, to: &str) -> Option<f64> {
    let grams = match from {
        "g" | "gram" | "grams" => val,
        "kg" | "kilogram" | "kilograms" => val * 1000.0,
        "mg" | "milligram" | "milligrams" => val * 0.001,
        "lb" | "lbs" | "pound" | "pounds" => val * 453.59237,
        "oz" | "ounce" | "ounces" => val * 28.349523,
        "ton" | "tons" => val * 1_000_000.0,
        _ => return None,
    };

    match to {
        "g" | "gram" | "grams" => Some(grams),
        "kg" | "kilogram" | "kilograms" => Some(grams / 1000.0),
        "mg" | "milligram" | "milligrams" => Some(grams / 0.001),
        "lb" | "lbs" | "pound" | "pounds" => Some(grams / 453.59237),
        "oz" | "ounce" | "ounces" => Some(grams / 28.349523),
        "ton" | "tons" => Some(grams / 1_000_000.0),
        _ => None,
    }
}

fn convert_data_storage(val: f64, from: &str, to: &str) -> Option<f64> {
    let bytes = match from {
        "b" | "byte" | "bytes" => val,
        "kb" | "kilobyte" => val * 1024.0,
        "mb" | "megabyte" => val * 1024.0 * 1024.0,
        "gb" | "gigabyte" => val * 1024.0 * 1024.0 * 1024.0,
        "tb" | "terabyte" => val * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };

    match to {
        "b" | "byte" | "bytes" => Some(bytes),
        "kb" | "kilobyte" => Some(bytes / 1024.0),
        "mb" | "megabyte" => Some(bytes / (1024.0 * 1024.0)),
        "gb" | "gigabyte" => Some(bytes / (1024.0 * 1024.0 * 1024.0)),
        "tb" | "terabyte" => Some(bytes / (1024.0 * 1024.0 * 1024.0 * 1024.0)),
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// 4. Currency & Crypto Converter
// -----------------------------------------------------------------------------
fn parse_currency_conversion(input: &str) -> Option<ConverterResult> {
    let lower = input.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();

    // Pattern: <val> <curr_from> to/in <curr_to>
    // e.g. "100 usd in try", "50 eur to usd", "1 btc in usd"
    if parts.len() != 4 || (parts[2] != "to" && parts[2] != "in" && parts[2] != "=") {
        return None;
    }

    let val: f64 = parts[0].parse().ok()?;
    let from_curr = parts[1].to_uppercase();
    let to_curr = parts[3].to_uppercase();

    let rates = get_exchange_rates();
    let from_rate = rates.get(&from_curr)?;
    let to_rate = rates.get(&to_curr)?;

    // Rate is relative to 1 USD
    let usd_val = val / from_rate;
    let converted = usd_val * to_rate;

    Some(ConverterResult {
        id: format!("curr_{}_{}", from_curr, to_curr),
        title: format!("💱 Döviz: {:.2} {}", converted, to_curr),
        preview: format!(
            "{:.2} {} = {:.2} {} (Tahmini Kur)",
            val, from_curr, converted, to_curr
        ),
        copy_value: format_clean_f64(converted),
        badge: "Döviz Çevirici",
    })
}

fn get_exchange_rates() -> HashMap<String, f64> {
    let mut rates = HashMap::new();
    // Default baseline rates anchored to 1 USD
    rates.insert("USD".to_string(), 1.0);
    rates.insert("TRY".to_string(), 36.40);
    rates.insert("EUR".to_string(), 0.925);
    rates.insert("GBP".to_string(), 0.792);
    rates.insert("JPY".to_string(), 155.20);
    rates.insert("CAD".to_string(), 1.41);
    rates.insert("AUD".to_string(), 1.56);
    rates.insert("CHF".to_string(), 0.905);
    rates.insert("CNY".to_string(), 7.28);
    rates.insert("BTC".to_string(), 0.000011); // ~90k USD
    rates.insert("ETH".to_string(), 0.00037); // ~2.7k USD
    rates.insert("SOL".to_string(), 0.0055); // ~180 USD

    // Check if cached file exists
    let cache_file = get_app_storage_dir().join("exchange_rates.json");
    if let Ok(file) = File::open(cache_file) {
        let reader = BufReader::new(file);
        if let Ok(cached_rates) = serde_json::from_reader::<_, HashMap<String, f64>>(reader) {
            for (k, v) in cached_rates {
                rates.insert(k.to_uppercase(), v);
            }
        }
    }

    rates
}

fn format_clean_f64(num: f64) -> String {
    if num.fract() == 0.0 {
        format!("{}", num as i64)
    } else {
        let s = format!("{:.4}", num);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn search_converters(input: &str) -> Vec<SearchResult> {
    let matches = parse_converters(input);
    matches
        .into_iter()
        .map(|m| SearchResult {
            id: m.id,
            title: m.title,
            subtitle: m.preview,
            badge: m.badge.to_string(),
            score: 9_600,
            action: ResultAction::CopyToClipboard {
                text: m.copy_value.clone(),
                notify_msg: format!("Sonuç kopyalandı: {}", m.copy_value),
            },
            color_preview: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_conversions() {
        let res = parse_unit_conversion("100 km to miles");
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(res.title.contains("62.1371"));

        let temp_res = parse_unit_conversion("100 c to f");
        assert!(temp_res.is_some());
        assert!(temp_res.unwrap().title.contains("212.00"));

        let data_res = parse_unit_conversion("16 gb in mb");
        assert!(data_res.is_some());
        assert!(data_res.unwrap().title.contains("16384.00"));
    }

    #[test]
    fn test_currency_conversions() {
        let res = parse_currency_conversion("100 usd in try");
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(res.title.contains("TRY"));
    }

    #[test]
    fn test_epoch_conversion() {
        let res = parse_epoch_converter("unix 1700000000");
        assert!(res.is_some());
        assert!(res.unwrap().title.contains("2023-11-14"));
    }
}

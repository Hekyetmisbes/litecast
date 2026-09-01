// Developer, Designer & Quick Utility Tools for LiteCast.

use windows::Win32::Foundation::COLORREF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevToolMatch {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub copy_value: String,
    pub badge: &'static str,
    pub color_preview: Option<COLORREF>,
}

pub fn parse_dev_tools(input: &str) -> Vec<DevToolMatch> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();

    // 1. Color Parser (Hex / RGB)
    if let Some(col) = parse_color(trimmed) {
        results.push(col);
    }

    // 2. Base64 Encode / Decode
    if let Some(b64) = parse_base64(trimmed) {
        results.push(b64);
    }

    // 3. UUID / GUID Generator
    if trimmed.eq_ignore_ascii_case("uuid")
        || trimmed.eq_ignore_ascii_case("guid")
        || trimmed.eq_ignore_ascii_case("uuid v4")
    {
        let uuid = generate_uuid_v4();
        results.push(DevToolMatch {
            id: "dev_uuid".to_string(),
            title: format!("🆔 UUID v4: {}", uuid),
            preview: "Enter'a basarak panoya kopyalayın".to_string(),
            copy_value: uuid,
            badge: "UUID / GUID",
            color_preview: None,
        });
    }

    // 4. SHA-256 / MD5 Hash
    if let Some(hash_match) = parse_hash(trimmed) {
        results.push(hash_match);
    }

    // 5. String Length / Word Counter
    if let Some(rest) = trimmed.strip_prefix("len ") {
        let chars = rest.chars().count();
        let words = rest.split_whitespace().count();
        let bytes = rest.len();
        results.push(DevToolMatch {
            id: "dev_len".to_string(),
            title: format!(
                "📏 Metin: {} karakter, {} kelime ({} bayt)",
                chars, words, bytes
            ),
            preview: format!("Girdi: \"{}\"", rest),
            copy_value: format!("{}", chars),
            badge: "Sayaç",
            color_preview: None,
        });
    }

    // 6. JSON Formatter & Minifier
    if let Some(json_matches) = parse_json(trimmed) {
        results.extend(json_matches);
    }

    // 7. JWT (JSON Web Token) Decoder
    if let Some(jwt_match) = parse_jwt(trimmed) {
        results.push(jwt_match);
    }

    // 8. Lorem Ipsum Generator
    if let Some(lorem_match) = parse_lorem(trimmed) {
        results.push(lorem_match);
    }

    // 9. Cron Expression Parser
    if let Some(cron_match) = parse_cron(trimmed) {
        results.push(cron_match);
    }

    results
}

// ---------------- Color Utilities ----------------

fn parse_color(input: &str) -> Option<DevToolMatch> {
    let trimmed = input.trim();

    // HEX Color (e.g. #3b82f6 or 3b82f6 or #fff)
    if (trimmed.starts_with('#') && (trimmed.len() == 4 || trimmed.len() == 7))
        || (trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_hexdigit()))
    {
        let hex_str = trimmed.trim_start_matches('#');
        let (r, g, b) = if hex_str.len() == 3 {
            let r = u8::from_str_radix(&hex_str[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex_str[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex_str[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        } else if hex_str.len() == 6 {
            let r = u8::from_str_radix(&hex_str[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex_str[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex_str[4..6], 16).ok()?;
            (r, g, b)
        } else {
            return None;
        };

        let rgb_str = format!("rgb({}, {}, {})", r, g, b);
        let clean_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);

        return Some(DevToolMatch {
            id: "dev_color".to_string(),
            title: format!("🎨 Renk: {}  •  {}", clean_hex, rgb_str),
            preview: "Enter'a basarak RGB değerini kopyalayın".to_string(),
            copy_value: rgb_str,
            badge: "Renk Paleti",
            color_preview: Some(COLORREF(
                (r as u32) | ((g as u32) << 8) | ((b as u32) << 16),
            )),
        });
    }

    // RGB Color (e.g. rgb(37, 99, 235) or 37, 99, 235)
    let lower = trimmed.to_lowercase();
    if lower.starts_with("rgb(") && lower.ends_with(')') {
        let inside = &lower[4..lower.len() - 1];
        let parts: Vec<&str> = inside.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;

            let hex_str = format!("#{:02X}{:02X}{:02X}", r, g, b);
            return Some(DevToolMatch {
                id: "dev_color".to_string(),
                title: format!("🎨 Renk: {}  •  rgb({}, {}, {})", hex_str, r, g, b),
                preview: "Enter'a basarak HEX değerini kopyalayın".to_string(),
                copy_value: hex_str,
                badge: "Renk Paleti",
                color_preview: Some(COLORREF(
                    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16),
                )),
            });
        }
    }

    None
}

// ---------------- Base64 Utilities ----------------

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn parse_base64(input: &str) -> Option<DevToolMatch> {
    let trimmed = input.trim();

    if let Some(rest) = trimmed.strip_prefix("b64 e ") {
        let encoded = base64_encode(rest.as_bytes());
        return Some(DevToolMatch {
            id: "dev_b64_enc".to_string(),
            title: format!("🔒 Base64 Kodlandı: {}", encoded),
            preview: format!("Girdi: \"{}\"", rest),
            copy_value: encoded,
            badge: "Base64",
            color_preview: None,
        });
    }

    if let Some(rest) = trimmed.strip_prefix("b64 d ")
        && let Some(decoded) = base64_decode(rest)
    {
        let text = String::from_utf8_lossy(&decoded).to_string();
        return Some(DevToolMatch {
            id: "dev_b64_dec".to_string(),
            title: format!("🔓 Base64 Çözüldü: {}", text),
            preview: format!("Girdi: \"{}\"", rest),
            copy_value: text,
            badge: "Base64",
            color_preview: None,
        });
    }

    if let Some(rest) = trimmed.strip_prefix("b64 ") {
        let encoded = base64_encode(rest.as_bytes());
        return Some(DevToolMatch {
            id: "dev_b64".to_string(),
            title: format!("🔒 Base64 Kodlandı: {}", encoded),
            preview: format!("Girdi: \"{}\"", rest),
            copy_value: encoded,
            badge: "Base64",
            color_preview: None,
        });
    }

    None
}

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as usize
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as usize
        } else {
            0
        };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[(triple >> 18) & 0x3F] as char);
        out.push(B64_CHARS[(triple >> 12) & 0x3F] as char);

        if i + 1 < data.len() {
            out.push(B64_CHARS[(triple >> 6) & 0x3F] as char);
        } else {
            out.push('=');
        }

        if i + 2 < data.len() {
            out.push(B64_CHARS[triple & 0x3F] as char);
        } else {
            out.push('=');
        }

        i += 3;
    }
    out
}

pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !clean.len().is_multiple_of(4) {
        return None;
    }

    let mut out = Vec::new();
    let mut i = 0;

    let decode_byte = |b: u8| -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    };

    while i < clean.len() {
        let b0 = decode_byte(clean[i])? as usize;
        let b1 = decode_byte(clean[i + 1])? as usize;
        let b2 = decode_byte(clean[i + 2])? as usize;
        let b3 = decode_byte(clean[i + 3])? as usize;

        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        out.push(((triple >> 16) & 0xFF) as u8);
        if clean[i + 2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if clean[i + 3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }

        i += 4;
    }

    Some(out)
}

// ---------------- UUID v4 Generator ----------------

pub fn generate_uuid_v4() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u128;
    let mut seed = now ^ (pid << 48);

    let mut rand_byte = || -> u8 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 56) as u8
    };

    let mut bytes = [0u8; 16];
    for b in bytes.iter_mut() {
        *b = rand_byte();
    }

    // Set version 4
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    // Set variant RFC 4122
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

// ---------------- Hash Utilities (SHA-256 / MD5) ----------------

fn parse_hash(input: &str) -> Option<DevToolMatch> {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("sha256 ") {
        let hash = sha256_hex(rest.as_bytes());
        return Some(DevToolMatch {
            id: "dev_sha256".to_string(),
            title: format!("🔑 SHA-256: {}", hash),
            preview: format!("Girdi: \"{}\"", rest),
            copy_value: hash,
            badge: "SHA-256",
            color_preview: None,
        });
    }

    if let Some(rest) = trimmed.strip_prefix("hash ") {
        let hash = sha256_hex(rest.as_bytes());
        return Some(DevToolMatch {
            id: "dev_hash".to_string(),
            title: format!("🔑 Hash (SHA-256): {}", hash),
            preview: format!("Girdi: \"{}\"", rest),
            copy_value: hash,
            badge: "SHA-256",
            color_preview: None,
        });
    }

    None
}

// Compact, standard SHA-256 implementation
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while !(msg.len() + 8).is_multiple_of(64) {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, w_val) in w.iter_mut().enumerate().take(16) {
            *w_val = u32::from_be_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_var = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_var
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_var = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_var);
    }

    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

// ---------------- JSON / JWT / Lorem / Cron Utilities ----------------

fn parse_json(input: &str) -> Option<Vec<DevToolMatch>> {
    let trimmed = input.trim();
    let json_str = if let Some(rest) = trimmed.strip_prefix("json ") {
        rest.trim()
    } else if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        trimmed
    } else {
        return None;
    };

    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let pretty = serde_json::to_string_pretty(&parsed).ok()?;
    let minified = serde_json::to_string(&parsed).ok()?;

    let preview = if pretty.len() > 70 {
        format!("{}...", pretty[..67].replace('\n', " "))
    } else {
        pretty.replace('\n', " ")
    };

    Some(vec![
        DevToolMatch {
            id: "dev_json_pretty".to_string(),
            title: "📋 JSON Formatla (Pretty)".to_string(),
            preview: format!("{}  •  Enter ile kopyalayın", preview),
            copy_value: pretty,
            badge: "JSON",
            color_preview: None,
        },
        DevToolMatch {
            id: "dev_json_minify".to_string(),
            title: "📦 JSON Küçült (Minify)".to_string(),
            preview: format!("{}  •  Enter ile kopyalayın", minified),
            copy_value: minified,
            badge: "JSON",
            color_preview: None,
        },
    ])
}

fn parse_jwt(input: &str) -> Option<DevToolMatch> {
    let trimmed = input.trim();
    let token = if let Some(rest) = trimmed.strip_prefix("jwt ") {
        rest.trim()
    } else if trimmed.starts_with("eyJ") && trimmed.split('.').count() == 3 {
        trimmed
    } else {
        return None;
    };

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let decode_part = |b64: &str| -> Option<String> {
        let mut padded = b64.replace('-', "+").replace('_', "/");
        while !padded.len().is_multiple_of(4) {
            padded.push('=');
        }
        let bytes = base64_decode(&padded)?;
        let json_str = String::from_utf8(bytes).ok()?;
        let v: serde_json::Value = serde_json::from_str(&json_str).ok()?;
        serde_json::to_string_pretty(&v).ok()
    };

    let header = decode_part(parts[0])?;
    let payload = decode_part(parts[1])?;

    let full_decoded = format!("// Header\n{}\n\n// Payload\n{}", header, payload);
    let preview = payload.lines().take(3).collect::<Vec<_>>().join(" ");

    Some(DevToolMatch {
        id: "dev_jwt".to_string(),
        title: "🔑 JWT Çözüldü (Decoded Payload)".to_string(),
        preview: format!("{}  •  Enter ile JSON'u kopyalayın", preview),
        copy_value: full_decoded,
        badge: "JWT",
        color_preview: None,
    })
}

fn parse_lorem(input: &str) -> Option<DevToolMatch> {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();

    if !trimmed.starts_with("lorem") {
        return None;
    }

    let standard_lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

    let words: Vec<&str> = standard_lorem.split_whitespace().collect();

    let (content, title) = if trimmed == "lorem" {
        (
            standard_lorem.to_string(),
            "📝 Lorem Ipsum (1 Paragraf)".to_string(),
        )
    } else if let Some(rest) = trimmed.strip_prefix("lorem ") {
        let r = rest.trim();
        if let Some(num_str) = r.strip_suffix('p').or_else(|| r.strip_suffix(" paragraf")) {
            let p_count: usize = num_str.trim().parse().unwrap_or(1).clamp(1, 10);
            let ps = vec![standard_lorem; p_count].join("\n\n");
            (ps, format!("📝 Lorem Ipsum ({} Paragraf)", p_count))
        } else if let Some(num_str) = r.strip_suffix('w').or_else(|| r.strip_suffix(" kelime")) {
            let w_count: usize = num_str.trim().parse().unwrap_or(20).clamp(1, 200);
            let mut gen_words = Vec::new();
            for i in 0..w_count {
                gen_words.push(words[i % words.len()]);
            }
            (
                gen_words.join(" "),
                format!("📝 Lorem Ipsum ({} Kelime)", w_count),
            )
        } else if let Ok(n) = r.parse::<usize>() {
            let w_count = n.clamp(1, 200);
            let mut gen_words = Vec::new();
            for i in 0..w_count {
                gen_words.push(words[i % words.len()]);
            }
            (
                gen_words.join(" "),
                format!("📝 Lorem Ipsum ({} Kelime)", w_count),
            )
        } else {
            (standard_lorem.to_string(), "📝 Lorem Ipsum".to_string())
        }
    } else {
        (standard_lorem.to_string(), "📝 Lorem Ipsum".to_string())
    };

    let preview = if content.len() > 60 {
        format!("{}...", &content[..57])
    } else {
        content.clone()
    };

    Some(DevToolMatch {
        id: "dev_lorem".to_string(),
        title,
        preview: format!("{}  •  Enter ile kopyalayın", preview),
        copy_value: content,
        badge: "Lorem Ipsum",
        color_preview: None,
    })
}

fn parse_cron(input: &str) -> Option<DevToolMatch> {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();

    let cron_str = {
        let rest = trimmed.strip_prefix("cron ")?;
        rest.trim()
    };

    let parts: Vec<&str> = cron_str.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }

    let min = parts[0];
    let hour = parts[1];
    let dom = parts[2];
    let mon = parts[3];
    let dow = parts[4];

    let mut desc = Vec::new();

    if min == "*" && hour == "*" {
        desc.push("Her dakika".to_string());
    } else if let Some(step) = min.strip_prefix("*/") {
        desc.push(format!("Her {} dakikada bir", step));
    } else if min == "0" && hour == "*" {
        desc.push("Her saat başı".to_string());
    } else if min == "0" && hour == "0" {
        desc.push("Her gece yarısı (00:00)".to_string());
    } else {
        desc.push(format!("Dakika {}, Saat {}", min, hour));
    }

    if dom != "*" {
        desc.push(format!("Ayın {}. günü", dom));
    }
    if mon != "*" {
        desc.push(format!("Ay: {}", mon));
    }
    if dow != "*" {
        let dow_name = match dow {
            "0" | "7" | "sun" => "Pazar",
            "1" | "mon" => "Pazartesi",
            "2" | "tue" => "Salı",
            "3" | "wed" => "Çarşamba",
            "4" | "thu" => "Perşembe",
            "5" | "fri" => "Cuma",
            "6" | "sat" => "Cumartesi",
            "1-5" | "mon-fri" => "Hafta içi (Pzt-Cum)",
            _ => dow,
        };
        desc.push(format!("Gün: {}", dow_name));
    }

    let full_desc = desc.join(", ");

    Some(DevToolMatch {
        id: "dev_cron".to_string(),
        title: format!("⏰ Cron Zamanlaması: {}", full_desc),
        preview: format!("İfade: \"{}\"  •  Enter ile kopyalayın", cron_str),
        copy_value: full_desc,
        badge: "Cron Parser",
        color_preview: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_parsing() {
        let res = parse_color("#3b82f6").unwrap();
        assert_eq!(res.copy_value, "rgb(59, 130, 246)");

        let res2 = parse_color("rgb(255, 99, 50)").unwrap();
        assert_eq!(res2.copy_value, "#FF6332");
    }

    #[test]
    fn test_base64() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        let decoded = base64_decode("aGVsbG8=").unwrap();
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn test_sha256() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_json_formatter() {
        let res = parse_json(r#"json {"name":"LiteCast","version":1}"#);
        assert!(res.is_some());
        let matches = res.unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches[0].copy_value.contains('\n'));
    }

    #[test]
    fn test_lorem() {
        let res = parse_lorem("lorem 20w");
        assert!(res.is_some());
        assert_eq!(res.unwrap().copy_value.split_whitespace().count(), 20);
    }

    #[test]
    fn test_cron() {
        let res = parse_cron("cron */15 * * * *");
        assert!(res.is_some());
        assert!(res.unwrap().title.contains("15"));
    }
}

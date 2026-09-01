// Web search bangs and browser fallback search provider for LiteCast.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebBangMatch {
    pub engine_name: &'static str,
    pub query: String,
    pub target_url: String,
    pub title: String,
    pub badge: &'static str,
}

pub struct BangDefinition {
    pub prefix: &'static str,
    pub name: &'static str,
    pub url_template: &'static str,
    pub badge: &'static str,
}

pub const BANGS: &[BangDefinition] = &[
    BangDefinition {
        prefix: "g",
        name: "Google",
        url_template: "https://www.google.com/search?q=",
        badge: "Google Arama",
    },
    BangDefinition {
        prefix: "yt",
        name: "YouTube",
        url_template: "https://www.youtube.com/results?search_query=",
        badge: "YouTube Arama",
    },
    BangDefinition {
        prefix: "gh",
        name: "GitHub",
        url_template: "https://github.com/search?q=",
        badge: "GitHub Arama",
    },
    BangDefinition {
        prefix: "wiki",
        name: "Wikipedia",
        url_template: "https://tr.wikipedia.org/wiki/Special:Search?search=",
        badge: "Vikipedi Arama",
    },
    BangDefinition {
        prefix: "ddg",
        name: "DuckDuckGo",
        url_template: "https://duckduckgo.com/?q=",
        badge: "DuckDuckGo",
    },
    BangDefinition {
        prefix: "tr",
        name: "Google Çeviri",
        url_template: "https://translate.google.com/?sl=auto&tl=tr&op=translate&text=",
        badge: "Çeviri",
    },
    BangDefinition {
        prefix: "chat",
        name: "ChatGPT",
        url_template: "https://chatgpt.com/?q=",
        badge: "AI Sohbet",
    },
    BangDefinition {
        prefix: "maps",
        name: "Google Haritalar",
        url_template: "https://www.google.com/maps/search/",
        badge: "Haritalar",
    },
    BangDefinition {
        prefix: "crates",
        name: "Crates.io (Rust)",
        url_template: "https://crates.io/search?q=",
        badge: "Crates.io",
    },
    BangDefinition {
        prefix: "npm",
        name: "NPM Package",
        url_template: "https://www.npmjs.com/search?q=",
        badge: "NPM Arama",
    },
    BangDefinition {
        prefix: "so",
        name: "StackOverflow",
        url_template: "https://stackoverflow.com/search?q=",
        badge: "StackOverflow",
    },
    BangDefinition {
        prefix: "reddit",
        name: "Reddit",
        url_template: "https://www.reddit.com/search/?q=",
        badge: "Reddit",
    },
];

pub fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

pub fn parse_web_bang(input: &str) -> Option<WebBangMatch> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.splitn(2, ' ');
    let first = parts.next()?.to_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    for bang in BANGS {
        if first == bang.prefix {
            let q = if rest.is_empty() {
                format!("{} Ana Sayfası", bang.name)
            } else {
                rest.to_string()
            };

            let target_url = if rest.is_empty() {
                // Strip query parameter prefix for base domain
                bang.url_template
                    .split('?')
                    .next()
                    .unwrap_or(bang.url_template)
                    .to_string()
            } else {
                format!("{}{}", bang.url_template, url_encode(rest))
            };

            let title = if rest.is_empty() {
                format!("🌐 {} sitesine git", bang.name)
            } else {
                format!("🌐 {}'da ara: \"{}\"", bang.name, rest)
            };

            return Some(WebBangMatch {
                engine_name: bang.name,
                query: q,
                target_url,
                title,
                badge: bang.badge,
            });
        }
    }

    None
}

pub fn make_web_fallback(input: &str) -> Option<WebBangMatch> {
    let trimmed = input.trim();
    if trimmed.len() < 2 {
        return None;
    }

    Some(WebBangMatch {
        engine_name: "Google",
        query: trimmed.to_string(),
        target_url: format!("https://www.google.com/search?q={}", url_encode(trimmed)),
        title: format!("🌐 Web'de Ara: \"{}\"", trimmed),
        badge: "Google Arama",
    })
}

pub fn parse_direct_url(input: &str) -> Option<WebBangMatch> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.contains(' ') {
        return None;
    }

    let lower = trimmed.to_lowercase();

    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(WebBangMatch {
            engine_name: "Web",
            query: trimmed.to_string(),
            target_url: trimmed.to_string(),
            title: format!("🌐 Web Sayfasını Aç: {}", trimmed),
            badge: "Web Sitesi",
        });
    }

    if lower.starts_with("localhost:") || lower.starts_with("127.0.0.1:") {
        let url = format!("http://{}", trimmed);
        return Some(WebBangMatch {
            engine_name: "Localhost",
            query: trimmed.to_string(),
            target_url: url,
            title: format!("🌐 Yerel Sunucuyu Aç: {}", trimmed),
            badge: "Yerel Web",
        });
    }

    // Common TLD check
    let tlds = [
        ".com", ".org", ".net", ".io", ".dev", ".app", ".ai", ".co", ".tr", ".edu", ".gov", ".me",
        ".xyz", ".cc", ".gg", ".to", ".info", ".tv",
    ];

    for tld in tlds {
        if lower.contains(tld) {
            let url = format!("https://{}", trimmed);
            return Some(WebBangMatch {
                engine_name: "Web",
                query: trimmed.to_string(),
                target_url: url,
                title: format!("🌐 Web Sayfasını Aç: {}", trimmed),
                badge: "Web Sitesi",
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_bang_parsing() {
        let res = parse_web_bang("g rust lang").unwrap();
        assert_eq!(res.engine_name, "Google");
        assert!(res.target_url.contains("q=rust+lang"));

        let res2 = parse_web_bang("yt lofi hip hop").unwrap();
        assert_eq!(res2.engine_name, "YouTube");
        assert!(res2.target_url.contains("lofi+hip+hop"));

        let res3 = parse_web_bang("gh litecast").unwrap();
        assert_eq!(res3.engine_name, "GitHub");

        let res4 = parse_web_bang("tr merhaba dunya").unwrap();
        assert_eq!(res4.engine_name, "Google Çeviri");
    }

    #[test]
    fn test_direct_url_parsing() {
        let res = parse_direct_url("github.com").unwrap();
        assert_eq!(res.target_url, "https://github.com");

        let res2 = parse_direct_url("https://crates.io/crates/windows").unwrap();
        assert_eq!(res2.target_url, "https://crates.io/crates/windows");

        let res3 = parse_direct_url("localhost:3000").unwrap();
        assert_eq!(res3.target_url, "http://localhost:3000");

        assert!(parse_direct_url("not a url").is_none());
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("c++ & rust!"), "c%2B%2B+%26+rust%21");
    }
}

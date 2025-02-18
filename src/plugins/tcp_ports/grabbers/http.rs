use std::time::Duration;

use crate::{plugins::tcp_ports::options, utils::net::StreamLike};
use lazy_static::lazy_static;
use regex::Regex;

use super::Banner;

// TODO: read from args
static HTTP_HEADERS_OF_INTEREST: &[&str] = &["server", "x-powered-by", "location"];

lazy_static! {
    static ref HTML_TITLE_PARSER: Regex =
        Regex::new(r"(?i)<\s*title\s*>([^<]+)<\s*/\s*title\s*>").unwrap();
}

pub(crate) fn is_http_port(opts: &options::Options, port: u16) -> (bool, bool) {
    for http_port in opts
        .tcp_ports_http
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if port == http_port.parse::<u16>().unwrap() {
            return (true, false);
        }
    }

    for https_port in opts
        .tcp_ports_https
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if port == https_port.parse::<u16>().unwrap() {
            return (true, true);
        }
    }

    (false, false)
}

pub(crate) async fn http_grabber(
    address: &str,
    port: u16,
    stream: Box<dyn StreamLike>,
    ssl: bool,
    timeout: Duration,
) -> Banner {
    let mut banner = Banner::default();
    let url = format!(
        "{}://{}:{}/",
        if ssl { "https" } else { "http" },
        address,
        port
    );

    drop(stream); // close original connection

    log::debug!("grabbing http banner for {} ...", &url);

    let cli = reqwest::Client::builder()
        .no_proxy() // used to set auto_sys_proxy to false, see https://github.com/evilsocket/legba/issues/8
        .danger_accept_invalid_certs(true)
        .build();
    let cli = if let Ok(c) = cli {
        c
    } else {
        log::error!(
            "can't create http client for {}:{}: {:?}",
            address,
            port,
            cli.err()
        );
        return banner;
    };

    let resp = cli
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/45.0.2454.85 Safari/537.36")
        .timeout(timeout)
        .send()
        .await;

    if let Ok(resp) = resp {
        // TODO: find a way to collect certificate information if ssl

        // collect headers
        for (name, value) in resp.headers() {
            let name = name.to_string();
            if HTTP_HEADERS_OF_INTEREST.contains(&name.as_str()) {
                banner.insert(name, value.to_str().unwrap().to_owned());
            }
        }

        // collect info from html
        let body = resp.text().await;
        if let Ok(body) = body {
            if let Some(caps) = HTML_TITLE_PARSER.captures(&body) {
                banner.insert("title".to_owned(), caps.get(1).unwrap().as_str().to_owned());
            }
        } else {
            log::error!(
                "can't read response body from {}:{}: {:?}",
                address,
                port,
                body.err()
            );
        }
    } else {
        log::error!(
            "can't connect via http client to {}:{}: {:?}",
            address,
            port,
            resp.err()
        );
    }

    banner
}

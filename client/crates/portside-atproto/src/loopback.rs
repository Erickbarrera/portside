//! The one-shot local web server the browser returns to after sign-in
//! (`http://127.0.0.1:<port>/callback`, RFC 8252 loopback redirect).

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

use crate::error::{AtprotoError, Result};
use crate::oauth::CallbackParams;

pub struct CallbackListener {
    listener: TcpListener,
    redirect_uri: Url,
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Portside</title>\
<body style=\"font-family:system-ui,sans-serif;background:#0F0E17;color:#E8EAEE;\
display:grid;place-items:center;height:90vh\"><div style=\"text-align:center\">\
<h1 style=\"font-weight:600\">{title}</h1><p>{body}</p></div>";

impl CallbackListener {
    /// Listens on a free port of 127.0.0.1. `redirect_uri` is the registered address
    /// without a port (e.g. `http://127.0.0.1/callback`); the port is filled in.
    pub async fn bind(redirect_uri: &Url) -> Result<CallbackListener> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let mut redirect_uri = redirect_uri.clone();
        redirect_uri
            .set_port(Some(port))
            .map_err(|()| AtprotoError::Resolve("bad redirect address".to_owned()))?;
        Ok(CallbackListener {
            listener,
            redirect_uri,
        })
    }

    pub fn redirect_uri(&self) -> &Url {
        &self.redirect_uri
    }

    /// Waits for the browser to arrive at the callback path. Other requests (a favicon)
    /// get a 404 and are ignored.
    pub async fn wait(self, timeout: Duration) -> Result<CallbackParams> {
        tokio::time::timeout(timeout, self.accept_callback())
            .await
            .map_err(|_| AtprotoError::Timeout)?
    }

    async fn accept_callback(&self) -> Result<CallbackParams> {
        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let mut buf = Vec::with_capacity(2048);
            let mut chunk = [0u8; 2048];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
                let n = stream.read(&mut chunk).await?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let request = String::from_utf8_lossy(&buf);
            let target = request
                .lines()
                .next()
                .and_then(|line| {
                    let mut parts = line.split(' ');
                    (parts.next() == Some("GET"))
                        .then(|| parts.next())
                        .flatten()
                })
                .unwrap_or("/");
            let url = self.redirect_uri.join(target).ok();
            let Some(url) = url.filter(|u| u.path() == self.redirect_uri.path()) else {
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
                continue;
            };
            let params = CallbackParams::from_url(&url);
            let (title, body) = if params.error.is_some() {
                (
                    "Sign-in didn't finish",
                    "You can close this tab and try again in Portside.",
                )
            } else {
                (
                    "You're signed in",
                    "You can close this tab and go back to Portside.",
                )
            };
            let page = DONE_PAGE.replace("{title}", title).replace("{body}", body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{page}",
                page.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
            return Ok(params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_the_callback_query() {
        let base = Url::parse("http://127.0.0.1/callback").expect("url");
        let listener = CallbackListener::bind(&base).await.expect("bind");
        let uri = listener.redirect_uri().clone();
        let waiter = tokio::spawn(listener.wait(Duration::from_secs(5)));

        let client = reqwest::Client::new();
        let miss = client
            .get(format!(
                "http://127.0.0.1:{}/favicon.ico",
                uri.port().expect("port")
            ))
            .send()
            .await
            .expect("favicon");
        assert_eq!(miss.status(), 404);
        let page = client
            .get(format!(
                "{uri}?code=c1&state=s1&iss=https%3A%2F%2Fauth.example"
            ))
            .send()
            .await
            .expect("callback");
        assert!(page.text().await.expect("text").contains("signed in"));

        let params = waiter.await.expect("join").expect("params");
        assert_eq!(params.code.as_deref(), Some("c1"));
        assert_eq!(params.state.as_deref(), Some("s1"));
        assert_eq!(params.iss.as_deref(), Some("https://auth.example"));
    }
}

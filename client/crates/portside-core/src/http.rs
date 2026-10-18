//! The one way Portside makes HTTP clients, so every request checks certificates the same way.

/// A reqwest client builder for this platform. Desktop systems check certificates with the
/// OS's own verifier. On Android that verifier needs setup through JNI which Portside doesn't
/// do, so certificates are checked against Mozilla's roots (`webpki-root-certs`) instead
/// (client/docs/platforms.md, "Android").
pub fn client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|cert| reqwest::Certificate::from_der(cert).ok()),
    );
    builder
}

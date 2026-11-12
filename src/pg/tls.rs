//! Encrypting the connection to the engine's PostgreSQL. The address carries libpq's `sslmode`
//! and `sslrootcert`, which tokio-postgres doesn't read itself, so they are taken out here:
//! - `disable`: plain text.
//! - `prefer`: encrypted when the server offers it, its certificate unchecked.
//! - `require`: encrypted, the certificate unchecked unless `sslrootcert` names a CA file (then
//!   as `verify-ca`).
//! - `verify-ca`: the certificate signed by a trusted CA, whatever host it names: for a tunnel,
//!   or an address the certificate doesn't carry.
//! - `verify-full`: that, and issued for the host in the address.
//!
//! Without `sslmode`, an address on this machine (`localhost`, a loopback address, a socket) is
//! `prefer` and any other `verify-full`. libpq's own default, `prefer`, would let a server across
//! the network go unencrypted or be impersonated. The CAs trusted are the system's, or only those
//! in the `sslrootcert` file (`system` names the system's).
//!
//! No channel binding: the password exchange is SCRAM without `-PLUS`, inside the encryption.

use std::error::Error as StdError;
use std::future::Future;
use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context, Poll};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::verify_server_cert_signed_by_trust_anchor;
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, InvalidDnsNameError, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_postgres::config::{Host, SslMode};
use tokio_postgres::tls::{ChannelBinding, MakeTlsConnect, TlsConnect, TlsStream};

/// How much of the server's certificate is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Encrypted, the certificate taken as it comes (`prefer`, `require`).
    None,
    /// Signed by a trusted CA (`verify-ca`).
    Ca,
    /// Signed by a trusted CA and issued for the host (`verify-full`).
    Full,
}

/// An address split into what tokio-postgres reads and what this module does.
#[derive(Debug, PartialEq, Eq)]
pub struct Address {
    /// The address without `sslmode` and `sslrootcert`.
    pub url: String,
    mode: Option<String>,
    root: Option<String>,
}

impl Address {
    pub fn read(url: &str) -> Self {
        let url = url.trim();
        let (base, query) = url.split_once('?').unwrap_or((url, ""));
        let (mut mode, mut root, mut kept) = (None, None, Vec::new());
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            match pair.split_once('=').unwrap_or((pair, "")) {
                ("sslmode", v) => mode = Some(decode(v)),
                ("sslrootcert", v) => root = Some(decode(v)),
                _ => kept.push(pair),
            }
        }
        let url = if kept.is_empty() { base.to_owned() } else { format!("{base}?{}", kept.join("&")) };
        Self { url, mode, root }
    }

    /// The CA file the address names, `~` read as the home folder.
    fn root_file(&self) -> Option<PathBuf> {
        let root = self.root.as_deref().filter(|r| !r.is_empty() && *r != "system")?;
        match (root.strip_prefix("~/"), std::env::var_os("HOME")) {
            (Some(rest), Some(home)) => Some(Path::new(&home).join(rest)),
            _ => Some(PathBuf::from(root)),
        }
    }

    /// The handshake tokio-postgres runs, and what is checked of the certificate.
    pub fn plan(&self, hosts: &[Host]) -> Result<(SslMode, Check), String> {
        Ok(match self.mode.as_deref() {
            None if hosts.iter().all(on_this_machine) => (SslMode::Prefer, Check::None),
            None => (SslMode::Require, Check::Full),
            Some("disable") => (SslMode::Disable, Check::None),
            Some("allow" | "prefer") => (SslMode::Prefer, Check::None),
            Some("require") if self.root_file().is_some() => (SslMode::Require, Check::Ca),
            Some("require") => (SslMode::Require, Check::None),
            Some("verify-ca") => (SslMode::Require, Check::Ca),
            Some("verify-full") => (SslMode::Require, Check::Full),
            Some(other) => {
                return Err(format!(
                    "sslmode={other}는 알 수 없는 값입니다. disable, prefer, require, verify-ca, verify-full 중 하나를 적어 주세요."
                ));
            }
        })
    }

    /// The TLS for `check`, trusting the CA file the address names or the system's.
    pub fn connector(&self, check: Check) -> Result<Connector, String> {
        let roots = match self.root_file() {
            Some(path) => from_file(&path)?,
            None => SYSTEM.clone(),
        };
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?;
        let mut config = match check {
            Check::Full => builder.with_root_certificates(roots),
            Check::Ca | Check::None => builder.dangerous().with_custom_certificate_verifier(Arc::new(Lenient {
                roots: (check == Check::Ca).then_some(roots),
                provider,
            })),
        }
        .with_no_client_auth();
        // PostgreSQL 17 checks the protocol named when a client names one.
        config.alpn_protocols = vec![b"postgresql".to_vec()];
        Ok(Connector(Arc::new(config)))
    }
}

/// `localhost`, a loopback address or a socket.
#[allow(irrefutable_let_patterns)] // Windows has no sockets in `Host`.
fn on_this_machine(host: &Host) -> bool {
    let Host::Tcp(name) = host else { return true };
    name.eq_ignore_ascii_case("localhost")
        || name.trim_matches(['[', ']']).parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// A query value's `%XX` read back, as tokio-postgres reads the rest of the address.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The CAs this Mac (or PC) trusts, read once; the bundled list if none could be read.
static SYSTEM: LazyLock<Arc<RootCertStore>> = LazyLock::new(|| {
    let mut roots = RootCertStore::empty();
    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    if roots.is_empty() {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    Arc::new(roots)
});

fn from_file(path: &Path) -> Result<Arc<RootCertStore>, String> {
    let unreadable = |e: &dyn std::fmt::Display| format!("CA 파일 {}을 읽지 못했습니다 ({e}).", path.display());
    let certs = CertificateDer::pem_file_iter(path)
        .and_then(|certs| certs.collect::<Result<Vec<_>, _>>())
        .map_err(|e| unreadable(&e))?;
    let mut roots = RootCertStore::empty();
    if roots.add_parsable_certificates(certs).0 == 0 {
        return Err(format!("CA 파일 {}에 인증서가 없습니다.", path.display()));
    }
    Ok(Arc::new(roots))
}

/// `verify-ca`, or no check at all: the handshake's signatures are still verified, so the
/// server holds the key of the certificate it shows.
#[derive(Debug)]
struct Lenient {
    roots: Option<Arc<RootCertStore>>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Lenient {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _name: &ServerName<'_>,
        _ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if let Some(roots) = &self.roots {
            let cert = ParsedCertificate::try_from(end_entity)?;
            verify_server_cert_signed_by_trust_anchor(
                &cert,
                roots,
                intermediates,
                now,
                self.provider.signature_verification_algorithms.all,
            )?;
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// The TLS tokio-postgres hands its socket to.
#[derive(Clone)]
pub struct Connector(Arc<ClientConfig>);

impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> MakeTlsConnect<S> for Connector {
    type Stream = Stream<S>;
    type TlsConnect = Connect;
    type Error = InvalidDnsNameError;

    fn make_tls_connect(&mut self, host: &str) -> Result<Connect, InvalidDnsNameError> {
        Ok(Connect { config: self.0.clone(), name: ServerName::try_from(host.to_owned())? })
    }
}

pub struct Connect {
    config: Arc<ClientConfig>,
    name: ServerName<'static>,
}

type Handshake<S> = Pin<Box<dyn Future<Output = Result<Stream<S>, Box<dyn StdError + Send + Sync>>> + Send>>;

impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> TlsConnect<S> for Connect {
    type Stream = Stream<S>;
    type Error = Box<dyn StdError + Send + Sync>;
    type Future = Handshake<S>;

    fn connect(self, socket: S) -> Handshake<S> {
        Box::pin(async move {
            match tokio_rustls::TlsConnector::from(self.config).connect(self.name, socket).await {
                Ok(stream) => Ok(Stream(stream)),
                // The rustls error itself, so [`trouble`] finds it among the causes.
                Err(e) => {
                    let kind = e.kind();
                    Err(e.into_inner().unwrap_or_else(|| Box::new(io::Error::from(kind))))
                }
            }
        })
    }
}

pub struct Stream<S>(tokio_rustls::client::TlsStream<S>);

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for Stream<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for Stream<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> TlsStream for Stream<S> {
    fn channel_binding(&self) -> ChannelBinding {
        ChannelBinding::none()
    }
}

/// What went wrong with the encryption, for the person at the settings, if that is what failed.
pub fn trouble(e: &tokio_postgres::Error, host: &str) -> Option<String> {
    let mut cause = e.source();
    while let Some(c) = cause {
        if let Some(r) = c.downcast_ref::<rustls::Error>() {
            return Some(match r {
                rustls::Error::InvalidCertificate(CertificateError::UnknownIssuer) => format!(
                    "{host}의 인증서를 발급한 곳(CA)을 이 컴퓨터가 믿지 않습니다. 클라우드가 준 CA 파일을 \
                     주소 끝에 ?sslrootcert=파일경로 로 붙여 주세요."
                ),
                rustls::Error::InvalidCertificate(
                    CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
                ) => format!(
                    "{host}의 인증서가 이 주소를 위한 것이 아닙니다. 터널이나 IP 주소로 붙었다면 \
                     주소 끝에 ?sslmode=verify-ca 를 붙여 주세요."
                ),
                rustls::Error::InvalidCertificate(
                    CertificateError::Expired | CertificateError::ExpiredContext { .. },
                ) => format!("{host}의 인증서는 유효 기간이 지났습니다."),
                other => format!("{host}와 암호화 연결을 맺지 못했습니다 ({other})."),
            });
        }
        if c.to_string() == "server does not support TLS" {
            return Some(format!(
                "{host}의 PostgreSQL은 암호화 연결을 받지 않습니다. 이 컴퓨터나 사설망 안의 서버라면 \
                 주소 끝에 ?sslmode=disable 을 붙여 주세요."
            ));
        }
        cause = c.source();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(url: &str) -> Vec<Host> {
        url.parse::<tokio_postgres::Config>().unwrap().get_hosts().to_vec()
    }

    #[test]
    fn the_address_gives_up_what_tokio_postgres_cannot_read() {
        let a = Address::read("postgres://u:p@db.example.com/wh?sslmode=verify-full&sslrootcert=%2Ftmp%2Fca.pem&application_name=x");
        assert_eq!(a.url, "postgres://u:p@db.example.com/wh?application_name=x");
        assert_eq!(a.mode.as_deref(), Some("verify-full"));
        assert_eq!(a.root_file(), Some(PathBuf::from("/tmp/ca.pem")));
        let a = Address::read("postgres://db.example.com/wh?sslmode=require");
        assert_eq!(a.url, "postgres://db.example.com/wh");
        assert!(a.url.parse::<tokio_postgres::Config>().is_ok());
    }

    #[test]
    fn an_address_across_the_network_is_checked_unless_it_says_otherwise() {
        let plan = |url: &str| {
            let a = Address::read(url);
            a.plan(&hosts(&a.url)).unwrap()
        };
        assert_eq!(plan("postgres://db.example.com/wh"), (SslMode::Require, Check::Full));
        assert_eq!(plan("postgres://10.0.0.5/wh"), (SslMode::Require, Check::Full));
        assert_eq!(plan("postgres://127.0.0.1:5432/wh"), (SslMode::Prefer, Check::None));
        assert_eq!(plan("postgres://localhost/wh"), (SslMode::Prefer, Check::None));
        assert_eq!(plan("postgres://[::1]:5432/wh"), (SslMode::Prefer, Check::None));
        assert_eq!(plan("postgres://db.example.com/wh?sslmode=disable"), (SslMode::Disable, Check::None));
        assert_eq!(plan("postgres://db.example.com/wh?sslmode=require"), (SslMode::Require, Check::None));
        assert_eq!(
            plan("postgres://db.example.com/wh?sslmode=require&sslrootcert=/tmp/ca.pem"),
            (SslMode::Require, Check::Ca)
        );
        assert_eq!(
            plan("postgres://db.example.com/wh?sslmode=require&sslrootcert=system"),
            (SslMode::Require, Check::None)
        );
        assert_eq!(plan("postgres://127.0.0.1/wh?sslmode=verify-ca"), (SslMode::Require, Check::Ca));
        let a = Address::read("postgres://db/wh?sslmode=strict");
        assert!(a.plan(&hosts(&a.url)).is_err());
    }

    #[test]
    fn the_ca_file_is_found_from_the_home_folder() {
        let a = Address::read("postgres://db/wh?sslrootcert=~/certs/ca.pem");
        let home = std::env::var_os("HOME").unwrap();
        assert_eq!(a.root_file(), Some(Path::new(&home).join("certs/ca.pem")));
    }

    #[test]
    fn a_missing_ca_file_is_named() {
        let a = Address::read("postgres://db/wh?sslrootcert=/nonexistent/ca.pem");
        let e = a.connector(Check::Full).err().unwrap();
        assert!(e.contains("/nonexistent/ca.pem"), "{e}");
    }
}

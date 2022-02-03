/// Source code from: https://github.com/rustls/rustls/tree/v/0.20.2/rustls/tests

use std::convert::TryInto;
use std::io;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use rustls_pemfile;

use rustls::Error;
use rustls::RootCertStore;
use rustls::{Certificate, PrivateKey};
use rustls::{ClientConfig, ClientConnection};
use rustls::{ConnectionCommon, ServerConfig, ServerConnection, SideData};

macro_rules! embed_files {
    (
        $(
            ($name:ident, $keytype:expr, $path:expr);
        )+
    ) => {
        $(
            const $name: &'static [u8] = include_bytes!(
                concat!(env!("CARGO_MANIFEST_DIR"), "/data/", $keytype, "/", $path));
        )+

        pub fn bytes_for(keytype: &str, path: &str) -> &'static [u8] {
            match (keytype, path) {
                $(
                    ($keytype, $path) => $name,
                )+
                _ => panic!("unknown keytype {} with path {}", keytype, path),
            }
        }
    }
}

embed_files! {
    (RSA_CA_CERT, "rsa", "ca.cert");
    (RSA_END_FULLCHAIN, "rsa", "end.fullchain");
    (RSA_END_KEY, "rsa", "end.key");
}

fn transfer(
    left: &mut (impl DerefMut + Deref<Target = ConnectionCommon<impl SideData>>),
    right: &mut (impl DerefMut + Deref<Target = ConnectionCommon<impl SideData>>),
) -> usize {
    let mut buf = [0u8; 262144];
    let mut total = 0;

    while left.wants_write() {
        let sz = {
            let into_buf: &mut dyn io::Write = &mut &mut buf[..];
            left.write_tls(into_buf).unwrap()
        };
        total += sz;
        if sz == 0 {
            return total;
        }

        let mut offs = 0;
        loop {
            let from_buf: &mut dyn io::Read = &mut &buf[offs..sz];
            offs += right.read_tls(from_buf).unwrap();
            if sz == offs {
                break;
            }
        }
    }

    total
}

#[derive(Clone, Copy, PartialEq)]
pub enum KeyType {
    RSA,
    ECDSA,
    ED25519,
}

impl KeyType {
    fn bytes_for(&self, part: &str) -> &'static [u8] {
        match self {
            KeyType::RSA => bytes_for("rsa", part),
            KeyType::ECDSA => bytes_for("ecdsa", part),
            KeyType::ED25519 => bytes_for("eddsa", part),
        }
    }

    fn get_chain(&self) -> Vec<Certificate> {
        rustls_pemfile::certs(&mut io::BufReader::new(self.bytes_for("end.fullchain")))
            .unwrap()
            .iter()
            .map(|v| Certificate(v.clone()))
            .collect()
    }

    fn get_key(&self) -> PrivateKey {
        PrivateKey(
            rustls_pemfile::pkcs8_private_keys(&mut io::BufReader::new(self.bytes_for("end.key")))
                .unwrap()[0]
                .clone(),
        )
    }
}

fn finish_server_config(
    kt: KeyType,
    conf: rustls::ConfigBuilder<ServerConfig, rustls::WantsVerifier>,
) -> ServerConfig {
    conf.with_no_client_auth()
        .with_single_cert(kt.get_chain(), kt.get_key())
        .unwrap()
}

pub fn make_server_config_with_versions(
    kt: KeyType,
    versions: &[&'static rustls::SupportedProtocolVersion],
) -> ServerConfig {
    finish_server_config(
        kt,
        ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(versions)
            .unwrap(),
    )
}

fn finish_client_config(
    kt: KeyType,
    config: rustls::ConfigBuilder<ClientConfig, rustls::WantsVerifier>,
) -> ClientConfig {
    let mut root_store = RootCertStore::empty();
    let mut rootbuf = io::BufReader::new(kt.bytes_for("ca.cert"));
    root_store.add_parsable_certificates(&rustls_pemfile::certs(&mut rootbuf).unwrap());

    config
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

pub fn make_client_config_with_versions(
    kt: KeyType,
    versions: &[&'static rustls::SupportedProtocolVersion],
) -> ClientConfig {
    let builder = ClientConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_protocol_versions(versions)
        .unwrap();
    finish_client_config(kt, builder)
}

pub fn make_pair_for_configs(
    client_config: ClientConfig,
    server_config: ServerConfig,
) -> (ClientConnection, ServerConnection) {
    make_pair_for_arc_configs(&Arc::new(client_config), &Arc::new(server_config))
}

fn make_pair_for_arc_configs(
    client_config: &Arc<ClientConfig>,
    server_config: &Arc<ServerConfig>,
) -> (ClientConnection, ServerConnection) {
    (
        ClientConnection::new(Arc::clone(&client_config), dns_name("localhost")).unwrap(),
        ServerConnection::new(Arc::clone(server_config)).unwrap(),
    )
}

pub fn do_handshake(
    client: &mut (impl DerefMut + Deref<Target = ConnectionCommon<impl SideData>>),
    server: &mut (impl DerefMut + Deref<Target = ConnectionCommon<impl SideData>>),
) -> (usize, usize) {
    let (mut to_client, mut to_server) = (0, 0);
    while server.is_handshaking() || client.is_handshaking() {
        to_server += transfer(client, server);
        server.process_new_packets().unwrap();
        to_client += transfer(server, client);
        client.process_new_packets().unwrap();
    }
    (to_server, to_client)
}

#[derive(PartialEq, Debug)]
pub enum ErrorFromPeer {
    Client(Error),
    Server(Error),
}

pub fn do_handshake_until_error(
    client: &mut ClientConnection,
    server: &mut ServerConnection,
) -> Result<(), ErrorFromPeer> {
    while server.is_handshaking() || client.is_handshaking() {
        transfer(client, server);
        server
            .process_new_packets()
            .map_err(|err| ErrorFromPeer::Server(err))?;
        transfer(server, client);
        client
            .process_new_packets()
            .map_err(|err| ErrorFromPeer::Client(err))?;
    }

    Ok(())
}

fn dns_name(name: &'static str) -> rustls::ServerName {
    name.try_into().unwrap()
}

#[cfg(test)]
mod test {
    use super::*;

    use rustls::{self, ProtocolVersion};

    fn version_test(
        client_versions: &[&'static rustls::SupportedProtocolVersion],
        server_versions: &[&'static rustls::SupportedProtocolVersion],
        result: Option<ProtocolVersion>,
    ) {
        let client_versions = if client_versions.is_empty() {
            &rustls::ALL_VERSIONS
        } else {
            client_versions
        };
        let server_versions = if server_versions.is_empty() {
            &rustls::ALL_VERSIONS
        } else {
            server_versions
        };

        let client_config = make_client_config_with_versions(KeyType::RSA, client_versions);
        let server_config = make_server_config_with_versions(KeyType::RSA, server_versions);

        println!(
            "version {:?} {:?} -> {:?}",
            client_versions, server_versions, result
        );

        let (mut client, mut server) = make_pair_for_configs(client_config, server_config);

        assert_eq!(client.protocol_version(), None);
        assert_eq!(server.protocol_version(), None);
        if result.is_none() {
            let err = do_handshake_until_error(&mut client, &mut server);
            assert!(err.is_err());
        } else {
            do_handshake(&mut client, &mut server);
            assert_eq!(client.protocol_version(), result);
            assert_eq!(server.protocol_version(), result);
        }
    }

    #[test]
    fn versions() {
        // default -> 1.3
        version_test(&[], &[], Some(ProtocolVersion::TLSv1_3));
    }
}

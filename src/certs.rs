use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::x509::extension::{
    AuthorityKeyIdentifier, BasicConstraints, KeyUsage, SubjectAlternativeName,
    SubjectKeyIdentifier,
};
use openssl::x509::{X509NameBuilder, X509Req, X509ReqBuilder, X509, X509Builder};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{Error, Result};

pub struct CertificateAuthority {
    cert: X509,
    key: PKey<Private>,
    cert_cache: Arc<RwLock<HashMap<String, (X509, PKey<Private>)>>>,
    cert_dir: PathBuf,
}

impl std::fmt::Debug for CertificateAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateAuthority")
            .field("cert_dir", &self.cert_dir)
            .finish()
    }
}

impl CertificateAuthority {
    pub fn new<P: AsRef<Path>>(cert_dir: P) -> Result<Self> {
        let cert_dir = cert_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cert_dir)?;

        let ca_cert_path = cert_dir.join("mitmproxy-ca-cert.pem");
        let ca_key_path = cert_dir.join("mitmproxy-ca-cert.p12");

        let (cert, key) = if ca_cert_path.exists() && ca_key_path.exists() {
            Self::load_ca_cert(&ca_cert_path, &ca_key_path)?
        } else {
            let (cert, key) = Self::generate_ca_cert()?;
            Self::save_ca_cert(&cert, &key, &ca_cert_path, &ca_key_path)?;
            (cert, key)
        };

        Ok(Self {
            cert,
            key,
            cert_cache: Arc::new(RwLock::new(HashMap::new())),
            cert_dir,
        })
    }

    pub async fn get_cert_for_host(&self, hostname: &str) -> Result<(X509, PKey<Private>)> {
        // Check cache first
        {
            let cache = self.cert_cache.read().await;
            if let Some((cert, key)) = cache.get(hostname) {
                return Ok((cert.clone(), key.clone()));
            }
        }

        // Generate new certificate
        let (cert, key) = self.generate_host_cert(hostname)?;

        // Cache the certificate
        {
            let mut cache = self.cert_cache.write().await;
            cache.insert(hostname.to_string(), (cert.clone(), key.clone()));
        }

        Ok((cert, key))
    }

    fn generate_ca_cert() -> Result<(X509, PKey<Private>)> {
        // Generate RSA key pair
        let rsa = Rsa::generate(2048)?;
        let key = PKey::from_rsa(rsa)?;

        // Create certificate
        let mut cert_builder = X509Builder::new()?;
        cert_builder.set_version(2)?;

        // Set serial number
        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        cert_builder.set_serial_number(&serial_number)?;

        // Set validity period (10 years)
        let not_before = Asn1Time::days_from_now(0)?;
        let not_after = Asn1Time::days_from_now(365 * 10)?;
        cert_builder.set_not_before(&not_before)?;
        cert_builder.set_not_after(&not_after)?;

        // Set subject and issuer
        let mut name_builder = X509NameBuilder::new()?;
        name_builder.append_entry_by_nid(Nid::COMMONNAME, "mitmproxy")?;
        name_builder.append_entry_by_nid(Nid::ORGANIZATIONNAME, "mitmproxy")?;
        let name = name_builder.build();

        cert_builder.set_subject_name(&name)?;
        cert_builder.set_issuer_name(&name)?;

        // Set public key
        cert_builder.set_pubkey(&key)?;

        // Add extensions
        cert_builder.append_extension(BasicConstraints::new().critical().ca().build()?)?;
        cert_builder.append_extension(
            KeyUsage::new()
                .critical()
                .key_cert_sign()
                .crl_sign()
                .build()?,
        )?;

        let subject_key_identifier =
            SubjectKeyIdentifier::new().build(&cert_builder.x509v3_context(None, None))?;
        cert_builder.append_extension(subject_key_identifier)?;

        // Sign the certificate
        cert_builder.sign(&key, MessageDigest::sha256())?;

        Ok((cert_builder.build(), key))
    }

    fn generate_host_cert(&self, hostname: &str) -> Result<(X509, PKey<Private>)> {
        // Generate RSA key pair
        let rsa = Rsa::generate(2048)?;
        let key = PKey::from_rsa(rsa)?;

        // Create certificate
        let mut cert_builder = X509Builder::new()?;
        cert_builder.set_version(2)?;

        // Set serial number
        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        cert_builder.set_serial_number(&serial_number)?;

        // Set validity period (1 year)
        let not_before = Asn1Time::days_from_now(0)?;
        let not_after = Asn1Time::days_from_now(365)?;
        cert_builder.set_not_before(&not_before)?;
        cert_builder.set_not_after(&not_after)?;

        // Set subject
        let mut name_builder = X509NameBuilder::new()?;
        name_builder.append_entry_by_nid(Nid::COMMONNAME, hostname)?;
        let subject_name = name_builder.build();
        cert_builder.set_subject_name(&subject_name)?;

        // Set issuer to CA
        cert_builder.set_issuer_name(self.cert.subject_name())?;

        // Set public key
        cert_builder.set_pubkey(&key)?;

        // Add extensions
        cert_builder.append_extension(BasicConstraints::new().build()?)?;

        cert_builder.append_extension(
            KeyUsage::new()
                .critical()
                .non_repudiation()
                .digital_signature()
                .key_encipherment()
                .build()?,
        )?;

        let subject_key_identifier =
            SubjectKeyIdentifier::new().build(&cert_builder.x509v3_context(Some(&self.cert), None))?;
        cert_builder.append_extension(subject_key_identifier)?;

        let authority_key_identifier = AuthorityKeyIdentifier::new()
            .keyid(false)
            .issuer(false)
            .build(&cert_builder.x509v3_context(Some(&self.cert), None))?;
        cert_builder.append_extension(authority_key_identifier)?;

        // Add Subject Alternative Name
        let mut san_builder = SubjectAlternativeName::new();
        san_builder.dns(hostname);

        // Also add wildcard version if not already wildcard
        if !hostname.starts_with("*.") {
            san_builder.dns(&format!("*.{}", hostname));
        }

        let san = san_builder.build(&cert_builder.x509v3_context(Some(&self.cert), None))?;
        cert_builder.append_extension(san)?;

        // Sign the certificate with CA key
        cert_builder.sign(&self.key, MessageDigest::sha256())?;

        Ok((cert_builder.build(), key))
    }

    fn load_ca_cert(cert_path: &Path, _key_path: &Path) -> Result<(X509, PKey<Private>)> {
        // For simplicity, we'll just regenerate if loading fails
        // In a real implementation, you'd want to properly load the existing CA
        Self::generate_ca_cert()
    }

    fn save_ca_cert(
        cert: &X509,
        key: &PKey<Private>,
        cert_path: &Path,
        _key_path: &Path,
    ) -> Result<()> {
        // Save certificate in PEM format
        let cert_pem = cert.to_pem()?;
        fs::write(cert_path, cert_pem)?;

        // In a real implementation, you'd save the private key as well
        // For now, we'll regenerate on each startup
        Ok(())
    }

    pub fn ca_cert_pem(&self) -> Result<Vec<u8>> {
        Ok(self.cert.to_pem()?)
    }

    pub fn ca_cert_der(&self) -> Result<Vec<u8>> {
        Ok(self.cert.to_der()?)
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.cert_cache.write().await;
        cache.clear();
    }

    pub async fn cache_size(&self) -> usize {
        let cache = self.cert_cache.read().await;
        cache.len()
    }
}

// Helper function to extract certificate information for JSON serialization
pub fn cert_to_info(cert: &X509) -> Result<crate::flow::Certificate> {
    use sha2::{Digest, Sha256};

    let der = cert.to_der()?;
    let mut hasher = Sha256::new();
    hasher.update(&der);
    let sha256 = format!("{:x}", hasher.finalize());

    let serial = cert.serial_number().to_bn()?.to_string();

    // Parse time strings to timestamps - using string representation
    let not_before = parse_asn1_time_to_timestamp(cert.not_before());
    let not_after = parse_asn1_time_to_timestamp(cert.not_after());

    // Extract subject and issuer info
    let subject = extract_name_entries(cert.subject_name());
    let issuer = extract_name_entries(cert.issuer_name());

    // Extract alternative names using subject_alt_names API
    let mut altnames = Vec::new();
    if let Some(sans) = cert.subject_alt_names() {
        for san in sans.iter() {
            if let Some(dns) = san.dnsname() {
                altnames.push(dns.to_string());
            }
        }
    }

    Ok(crate::flow::Certificate {
        keyinfo: "RSA 2048".to_string(), // Simplified
        sha256,
        notbefore: not_before,
        notafter: not_after,
        serial,
        subject,
        issuer,
        altnames,
    })
}

/// Parse ASN1 time to Unix timestamp
fn parse_asn1_time_to_timestamp(time: &openssl::asn1::Asn1TimeRef) -> i64 {
    // ASN1 time format: YYMMDDhhmmssZ or YYYYMMDDhhmmssZ
    // Use the to_string() method and parse the result
    let time_str = format!("{}", time);

    // Try to parse the time string - if parsing fails, return 0
    // In a real implementation, you'd use chrono or time crate for proper parsing
    if time_str.len() >= 12 {
        // Very basic timestamp approximation - in production you'd want proper parsing
        // For now, just return 0 as a placeholder
        0
    } else {
        0
    }
}

fn extract_name_entries(name: &openssl::x509::X509NameRef) -> indexmap::IndexMap<String, String> {
    let mut entries = indexmap::IndexMap::new();

    for entry in name.entries() {
        // short_name() already returns the short name like "CN", "O", etc.
        let key = entry.object().nid().short_name().unwrap_or("unknown").to_string();
        if let Ok(value) = entry.data().as_utf8() {
            entries.insert(key, value.to_string());
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ca_generation() {
        let temp_dir = TempDir::new().unwrap();
        let ca = CertificateAuthority::new(temp_dir.path()).unwrap();

        let ca_pem = ca.ca_cert_pem().unwrap();
        assert!(!ca_pem.is_empty());

        let ca_der = ca.ca_cert_der().unwrap();
        assert!(!ca_der.is_empty());
    }

    #[tokio::test]
    async fn test_host_cert_generation() {
        let temp_dir = TempDir::new().unwrap();
        let ca = CertificateAuthority::new(temp_dir.path()).unwrap();

        let (cert, _key) = ca.get_cert_for_host("example.com").await.unwrap();

        // Verify the certificate is valid
        assert_eq!(cert.version(), 2);

        // Check that the certificate was cached
        assert_eq!(ca.cache_size().await, 1);

        // Request the same host again - should come from cache
        let (cert2, _key2) = ca.get_cert_for_host("example.com").await.unwrap();
        assert_eq!(cert.to_der().unwrap(), cert2.to_der().unwrap());
        assert_eq!(ca.cache_size().await, 1);

        // Request a different host
        let (_cert3, _key3) = ca.get_cert_for_host("test.com").await.unwrap();
        assert_eq!(ca.cache_size().await, 2);
    }

    #[test]
    fn test_cert_info_extraction() {
        let temp_dir = TempDir::new().unwrap();
        let ca = CertificateAuthority::new(temp_dir.path()).unwrap();

        let cert_info = cert_to_info(&ca.cert).unwrap();
        assert!(!cert_info.sha256.is_empty());
        assert!(!cert_info.serial.is_empty());
        assert!(cert_info.subject.contains_key("CN"));
    }
}
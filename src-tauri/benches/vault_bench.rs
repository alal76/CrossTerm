//! Performance benchmarks for vault operations (PERF-01 through PERF-03).
//!
//! Benchmarks:
//! - Vault creation time (Argon2id key derivation + DB init)
//! - Vault unlock time (Argon2id key derivation)
//! - Credential encrypt/decrypt throughput (AES-256-GCM)

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::RngCore;

const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

// ── Argon2id key derivation benchmark ───────────────────────────────────

fn bench_argon2id_derive(c: &mut Criterion) {
    let password = b"BenchmarkPassword123!";
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    // Use production Argon2id parameters: m=64MB, t=3, p=4
    let params = Params::new(65536, 3, 4, Some(KEY_LEN)).unwrap();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    c.bench_function("vault_creation_argon2id_derive", |b| {
        b.iter(|| {
            let mut key = vec![0u8; KEY_LEN];
            argon2
                .hash_password_into(black_box(password), &salt, &mut key)
                .unwrap();
            black_box(key);
        });
    });
}

fn bench_vault_unlock(c: &mut Criterion) {
    let password = b"UnlockPassword456!";
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    // Same parameters as production
    let params = Params::new(65536, 3, 4, Some(KEY_LEN)).unwrap();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    c.bench_function("vault_unlock_argon2id_derive", |b| {
        b.iter(|| {
            let mut key = vec![0u8; KEY_LEN];
            argon2
                .hash_password_into(black_box(password), &salt, &mut key)
                .unwrap();
            black_box(key);
        });
    });
}

// ── AES-256-GCM encrypt/decrypt throughput ──────────────────────────────

fn bench_credential_encrypt(c: &mut Criterion) {
    let mut key_bytes = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key_bytes);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).unwrap();

    // Typical credential JSON payload sizes
    let sizes: Vec<usize> = vec![64, 256, 1024, 4096];

    let mut group = c.benchmark_group("credential_encrypt");
    for size in &sizes {
        let plaintext: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut nonce_bytes = [0u8; NONCE_LEN];
                OsRng.fill_bytes(&mut nonce_bytes);
                let nonce = Nonce::from_slice(&nonce_bytes);
                let ct = cipher.encrypt(nonce, black_box(plaintext.as_ref())).unwrap();
                black_box(ct);
            });
        });
    }
    group.finish();
}

fn bench_credential_decrypt(c: &mut Criterion) {
    let mut key_bytes = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key_bytes);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).unwrap();

    let sizes: Vec<usize> = vec![64, 256, 1024, 4096];

    let mut group = c.benchmark_group("credential_decrypt");
    for size in &sizes {
        let plaintext: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let pt = cipher
                    .decrypt(nonce, black_box(ciphertext.as_ref()))
                    .unwrap();
                black_box(pt);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_argon2id_derive,
    bench_vault_unlock,
    bench_credential_encrypt,
    bench_credential_decrypt
);
criterion_main!(benches);

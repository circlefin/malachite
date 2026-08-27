use bytes::Bytes;
use core::mem::size_of;
use futures::executor::block_on;
use rand::{rngs::StdRng, SeedableRng};

use arc_malachitebft_test::{Address, Ed25519Signer, Height, TestContext, ValueId, Vote};
use malachitebft_core_types::{NilOrVal, Round, VoteExtensionScope};
use malachitebft_signing::{Signer, Verifier};
use malachitebft_signing_ed25519::PrivateKey;

fn make_signer(seed: u64) -> (Ed25519Signer, Address) {
    let mut rng = StdRng::seed_from_u64(seed);
    let private_key = PrivateKey::generate(&mut rng);
    let address = Address::from_public_key(&private_key.public_key());
    (Ed25519Signer::new(private_key), address)
}

fn scope(
    address: Address,
    height: u64,
    round: u32,
    value_id: u64,
) -> VoteExtensionScope<TestContext> {
    VoteExtensionScope::new(
        Height::new(height),
        Round::new(round),
        ValueId::new(value_id),
        address,
    )
}

#[test]
fn sign_then_verify_in_same_scope_is_valid() {
    let (signer, addr) = make_signer(0x10);
    let pubkey = signer.private_key().public_key();
    let scope = scope(addr, 7, 0, 42);
    let ext = Bytes::from_static(b"payload");

    let signed = block_on(signer.sign_vote_extension(scope.clone(), ext.clone())).unwrap();
    let result = block_on(signer.verify_signed_vote_extension(
        &scope,
        &signed.message,
        &signed.signature,
        &pubkey,
    ))
    .unwrap();

    assert!(result.is_valid());
}

#[test]
fn verify_rejects_extension_replayed_at_different_height() {
    let (signer, addr) = make_signer(0x11);
    let pubkey = signer.private_key().public_key();
    let signing_scope = scope(addr, 7, 0, 42);
    let other_scope = scope(addr, 8, 0, 42);
    let ext = Bytes::from_static(b"payload");

    let signed = block_on(signer.sign_vote_extension(signing_scope, ext)).unwrap();
    let result = block_on(signer.verify_signed_vote_extension(
        &other_scope,
        &signed.message,
        &signed.signature,
        &pubkey,
    ))
    .unwrap();

    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_extension_replayed_at_different_round() {
    let (signer, addr) = make_signer(0x12);
    let pubkey = signer.private_key().public_key();
    let signing_scope = scope(addr, 7, 0, 42);
    let other_scope = scope(addr, 7, 1, 42);
    let ext = Bytes::from_static(b"payload");

    let signed = block_on(signer.sign_vote_extension(signing_scope, ext)).unwrap();
    let result = block_on(signer.verify_signed_vote_extension(
        &other_scope,
        &signed.message,
        &signed.signature,
        &pubkey,
    ))
    .unwrap();

    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_extension_replayed_for_different_value_id() {
    let (signer, addr) = make_signer(0x13);
    let pubkey = signer.private_key().public_key();
    let signing_scope = scope(addr, 7, 0, 42);
    let other_scope = scope(addr, 7, 0, 43);
    let ext = Bytes::from_static(b"payload");

    let signed = block_on(signer.sign_vote_extension(signing_scope, ext)).unwrap();
    let result = block_on(signer.verify_signed_vote_extension(
        &other_scope,
        &signed.message,
        &signed.signature,
        &pubkey,
    ))
    .unwrap();

    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_extension_attributed_to_different_validator() {
    let (signer_a, addr_a) = make_signer(0x14);
    let (_, addr_b) = make_signer(0x15);
    let pubkey_a = signer_a.private_key().public_key();
    let signing_scope = scope(addr_a, 7, 0, 42);
    let other_scope = scope(addr_b, 7, 0, 42);
    let ext = Bytes::from_static(b"payload");

    let signed = block_on(signer_a.sign_vote_extension(signing_scope, ext)).unwrap();
    let result = block_on(signer_a.verify_signed_vote_extension(
        &other_scope,
        &signed.message,
        &signed.signature,
        &pubkey_a,
    ))
    .unwrap();

    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_tampered_extension_payload() {
    let (signer, addr) = make_signer(0x16);
    let pubkey = signer.private_key().public_key();
    let signing_scope = scope(addr, 7, 0, 42);

    let signed =
        block_on(signer.sign_vote_extension(signing_scope.clone(), Bytes::from_static(b"orig")))
            .unwrap();

    let tampered = Bytes::from_static(b"tampered");
    let result = block_on(signer.verify_signed_vote_extension(
        &signing_scope,
        &tampered,
        &signed.signature,
        &pubkey,
    ))
    .unwrap();

    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_signature_from_different_key() {
    let (signer_a, addr_a) = make_signer(0x17);
    let (signer_b, _addr_b) = make_signer(0x18);
    let pubkey_a = signer_a.private_key().public_key();
    let scope = scope(addr_a, 7, 0, 42);
    let ext = Bytes::from_static(b"payload");

    let signed_by_b = block_on(signer_b.sign_vote_extension(scope.clone(), ext)).unwrap();
    let result = block_on(signer_a.verify_signed_vote_extension(
        &scope,
        &signed_by_b.message,
        &signed_by_b.signature,
        &pubkey_a,
    ))
    .unwrap();

    assert!(result.is_invalid());
}

/// Regression coverage for vote-extension domain separation. If the
/// implementation stops including the vote-extension domain tag in the
/// verified preimage, this hand-crafted domainless signature would become
/// valid for the otherwise matching scope and extension.
#[test]
fn verify_rejects_domainless_vote_extension_signature() {
    const HEIGHT: u64 = 7;
    const ROUND: u32 = 0;
    const VALUE_ID: u64 = 42;

    let (signer, addr) = make_signer(0x19);
    let pubkey = signer.private_key().public_key();
    let scope = scope(addr, HEIGHT, ROUND, VALUE_ID);
    let ext = Bytes::from_static(b"payload");

    let precommit = Vote::new_precommit(
        Height::new(HEIGHT),
        Round::new(ROUND),
        NilOrVal::Val(ValueId::new(VALUE_ID)),
        addr,
    );
    let precommit_bytes = precommit.to_sign_bytes();

    let mut domainless_preimage =
        Vec::with_capacity(size_of::<u64>() + precommit_bytes.len() + size_of::<u64>() + ext.len());
    domainless_preimage.extend_from_slice(&(precommit_bytes.len() as u64).to_be_bytes());
    domainless_preimage.extend_from_slice(&precommit_bytes);
    domainless_preimage.extend_from_slice(&(ext.len() as u64).to_be_bytes());
    domainless_preimage.extend_from_slice(&ext);

    let domainless_signature = signer.sign(&domainless_preimage);
    let result =
        block_on(signer.verify_signed_vote_extension(&scope, &ext, &domainless_signature, &pubkey))
            .unwrap();

    assert!(result.is_invalid());
}

/// Defense-in-depth: the public signing API cannot produce a "nil-value
/// scope" signature — `VoteExtensionScope.value_id` is `ValueId<Ctx>`, not
/// `NilOrVal<ValueId<Ctx>>` — so an attacker would have to bypass the trait
/// and hand-craft a preimage with `NilOrVal::Nil` in the precommit's value
/// field. This test confirms that even such a hand-crafted signature does
/// not pass verification against any (val-shaped) scope, i.e. the
/// `NilOrVal` wrapper in the canonical preimage cannot be confused for an
/// honest val-scope signature.
#[test]
fn verify_rejects_manually_crafted_nil_value_extension_signature() {
    const HEIGHT: u64 = 7;
    const ROUND: u32 = 0;
    const VALUE_ID: u64 = 42;

    let (signer, addr) = make_signer(0x20);
    let pubkey = signer.private_key().public_key();
    let ext = Bytes::from_static(b"payload");

    // Hand-craft an extension preimage as if the precommit's value were Nil.
    // This mirrors the canonical envelope used by the Ed25519 implementation
    // (`b"malachitebft/vote-extension/v1\0" || precommit_len ||
    // precommit_sign_bytes || extension_len || ext`).
    let nil_precommit =
        Vote::new_precommit(Height::new(HEIGHT), Round::new(ROUND), NilOrVal::Nil, addr);
    let nil_precommit_bytes = nil_precommit.to_sign_bytes();

    let mut bogus_preimage = Vec::with_capacity(
        b"malachitebft/vote-extension/v1\0".len()
            + size_of::<u64>()
            + nil_precommit_bytes.len()
            + size_of::<u64>()
            + ext.len(),
    );
    bogus_preimage.extend_from_slice(b"malachitebft/vote-extension/v1\0");
    bogus_preimage.extend_from_slice(&(nil_precommit_bytes.len() as u64).to_be_bytes());
    bogus_preimage.extend_from_slice(&nil_precommit_bytes);
    bogus_preimage.extend_from_slice(&(ext.len() as u64).to_be_bytes());
    bogus_preimage.extend_from_slice(&ext);

    let bogus_signature = signer.sign(&bogus_preimage);

    // Try to verify the hand-crafted (nil-value) signature against a
    // val-shaped scope with the same height, round, and address.
    let scope = scope(addr, HEIGHT, ROUND, VALUE_ID);
    let result =
        block_on(signer.verify_signed_vote_extension(&scope, &ext, &bogus_signature, &pubkey))
            .unwrap();

    assert!(result.is_invalid());
}

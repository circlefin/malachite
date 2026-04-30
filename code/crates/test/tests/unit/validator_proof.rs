use futures::executor::block_on;
use rand::{rngs::StdRng, SeedableRng};

use arc_malachitebft_test::{Ed25519Signer, TestContext};
use malachitebft_core_types::ValidatorProof;
use malachitebft_signing::{Signer, Verifier};
use malachitebft_signing_ed25519::{PrivateKey, Signature};

const POV_SEPARATOR: &[u8] = b"PoV";

fn make_signer(seed: u64) -> (Ed25519Signer, Vec<u8>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let private_key = PrivateKey::generate(&mut rng);
    let public_key_bytes = private_key.public_key().as_bytes().to_vec();
    (Ed25519Signer::new(private_key), public_key_bytes)
}

fn make_proof(
    signer: &Ed25519Signer,
    public_key: Vec<u8>,
    peer_id: Vec<u8>,
) -> ValidatorProof<TestContext> {
    block_on(signer.sign_validator_proof(public_key, peer_id)).unwrap()
}

#[test]
fn preimage_matches_signing_bytes() {
    let (signer, pk_bytes) = make_signer(0xA);
    let peer = b"peer-id-bytes".to_vec();
    let proof = make_proof(&signer, pk_bytes.clone(), peer.clone());

    assert_eq!(
        proof.preimage(),
        ValidatorProof::<TestContext>::signing_bytes(&pk_bytes, &peer),
    );
}

#[test]
fn preimage_layout_is_separator_then_length_prefixed_fields() {
    let (signer, pk_bytes) = make_signer(0xB);
    let peer = b"test-peer".to_vec();
    let proof = make_proof(&signer, pk_bytes.clone(), peer.clone());

    let mut expected = Vec::new();
    expected.extend_from_slice(POV_SEPARATOR);
    expected.extend_from_slice(&(pk_bytes.len() as u32).to_be_bytes());
    expected.extend_from_slice(&pk_bytes);
    expected.extend_from_slice(&(peer.len() as u32).to_be_bytes());
    expected.extend_from_slice(&peer);

    assert_eq!(proof.preimage(), expected);
}

#[test]
fn sign_then_verify_is_valid() {
    let (signer, pk_bytes) = make_signer(0xE);
    let proof = make_proof(&signer, pk_bytes, b"peer-1".to_vec());

    let result = block_on(signer.verify_validator_proof(&proof)).unwrap();
    assert!(result.is_valid());
}

#[test]
fn verify_rejects_tampered_signature() {
    let (signer, pk_bytes) = make_signer(0xF);
    let proof = make_proof(&signer, pk_bytes, b"peer-1".to_vec());

    let mut sig_bytes = proof.signature.to_bytes();
    sig_bytes[0] ^= 0xff;
    let tampered = ValidatorProof::<TestContext>::new(
        proof.public_key,
        proof.peer_id,
        Signature::from_bytes(sig_bytes),
    );

    let result = block_on(signer.verify_validator_proof(&tampered)).unwrap();
    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_mismatched_public_key() {
    let (signer_a, pk_a) = make_signer(0x1);
    let (_signer_b, pk_b) = make_signer(0x2);

    let proof = make_proof(&signer_a, pk_a, b"peer".to_vec());
    let tampered = ValidatorProof::<TestContext>::new(pk_b, proof.peer_id, proof.signature);

    let result = block_on(signer_a.verify_validator_proof(&tampered)).unwrap();
    assert!(result.is_invalid());
}

#[test]
fn verify_rejects_mismatched_peer_id() {
    let (signer, pk_bytes) = make_signer(0x3);
    let proof = make_proof(&signer, pk_bytes, b"peer-original".to_vec());

    let tampered = ValidatorProof::<TestContext>::new(
        proof.public_key,
        b"peer-different".to_vec(),
        proof.signature,
    );

    let result = block_on(signer.verify_validator_proof(&tampered)).unwrap();
    assert!(result.is_invalid());
}

#[test]
fn verify_errors_on_malformed_public_key() {
    let (signer, _) = make_signer(0x4);
    let proof =
        ValidatorProof::<TestContext>::new(vec![0u8; 16], b"peer".to_vec(), Signature::test());

    assert!(block_on(signer.verify_validator_proof(&proof)).is_err());
}

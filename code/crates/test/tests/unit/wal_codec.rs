//! Round-trip and on-disk-format tests for the WAL frame codec
//! (`engine::wal::{encode_entry, decode_entry}`).
//!
//! These tests pin down:
//! - the tag-byte + u64-length-prefix framing applied to every persistable [`Input`] variant
//!   that delegates to the codec layer (`Vote`, `Proposal`, `ProposedValue`, `PolkaCertificate`);
//! - the compact custom format used for `TimeoutElapsed` (tag + step byte + i64 BE round);
//! - the stable tag values that determine on-disk backward compatibility;
//! - the rejection of [`Input`] variants that must not be persisted.

use std::io::Cursor;

use arc_malachitebft_test::codec::proto::ProtobufCodec;
use arc_malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Value, ValueId, Vote,
};

use malachitebft_core_consensus::{Input, LocallyProposedValue, ProposedValue};
use malachitebft_core_types::{
    NilOrVal, PolkaCertificate, PolkaSignature, Round, SignedProposal, SignedVote, Timeout,
    Validity, ValueOrigin,
};
use malachitebft_engine::wal::{decode_entry, encode_entry};

// Tag bytes that determine on-disk WAL backward compatibility.
// Changing any of these constants is a format-breaking change.
const TAG_CONSENSUS: u8 = 0x01;
const TAG_TIMEOUT: u8 = 0x02;
const TAG_PROPOSED_VALUE: u8 = 0x04;
const TAG_POLKA_CERTIFICATE: u8 = 0x08;

fn encode_to_bytes(entry: &Input<TestContext>) -> Vec<u8> {
    let codec = ProtobufCodec;
    let mut buf = Vec::new();
    encode_entry(entry.clone(), &codec, &mut buf).expect("encode_entry");
    buf
}

fn round_trip(entry: &Input<TestContext>) -> Input<TestContext> {
    let codec = ProtobufCodec;
    let buf = encode_to_bytes(entry);
    decode_entry(&codec, Cursor::new(buf)).expect("decode_entry")
}

#[test]
fn vote_round_trip() {
    let vote = SignedVote::new(
        Vote::new_prevote(
            Height::new(11),
            Round::new(2),
            NilOrVal::Val(ValueId::new(0xDEAD)),
            Address::new([9; 20]),
        ),
        Signature::test(),
    );

    let entry = Input::Vote(vote.clone());
    let bytes = encode_to_bytes(&entry);
    assert_eq!(
        bytes[0], TAG_CONSENSUS,
        "Vote must encode under TAG_CONSENSUS"
    );

    let decoded = round_trip(&entry);
    let Input::Vote(got) = decoded else {
        panic!("expected Vote variant, got {decoded:?}");
    };
    assert_eq!(got.message, vote.message);
    assert_eq!(got.signature.to_bytes(), vote.signature.to_bytes());
}

#[test]
fn proposal_round_trip() {
    let proposal = SignedProposal::new(
        Proposal::new(
            Height::new(11),
            Round::new(2),
            Value::new(0xBEEF),
            Round::new(1),
            Address::new([9; 20]),
        ),
        Signature::test(),
    );

    let entry = Input::Proposal(proposal.clone());
    let bytes = encode_to_bytes(&entry);
    assert_eq!(
        bytes[0], TAG_CONSENSUS,
        "Proposal must encode under TAG_CONSENSUS (shared with Vote)"
    );

    let decoded = round_trip(&entry);
    let Input::Proposal(got) = decoded else {
        panic!("expected Proposal variant, got {decoded:?}");
    };
    assert_eq!(got.message, proposal.message);
    assert_eq!(got.signature.to_bytes(), proposal.signature.to_bytes());
}

#[test]
fn timeout_round_trip() {
    for timeout in [
        Timeout::propose(Round::new(0)),
        Timeout::prevote(Round::new(1)),
        Timeout::precommit(Round::new(2)),
        Timeout::rebroadcast(Round::new(3)),
    ] {
        let entry = Input::TimeoutElapsed(timeout);
        let bytes = encode_to_bytes(&entry);
        assert_eq!(
            bytes[0], TAG_TIMEOUT,
            "Timeout must encode under TAG_TIMEOUT"
        );
        assert_eq!(
            bytes.len(),
            1 /* tag */ + 1 /* step */ + 8, /* round */
            "Timeout uses a fixed 10-byte custom format"
        );

        let decoded = round_trip(&entry);
        let Input::TimeoutElapsed(got) = decoded else {
            panic!("expected TimeoutElapsed variant, got {decoded:?}");
        };
        assert_eq!(got.kind, timeout.kind);
        assert_eq!(got.round, timeout.round);
    }
}

#[test]
fn proposed_value_round_trip() {
    let pv = ProposedValue::<TestContext> {
        height: Height::new(42),
        round: Round::new(0),
        valid_round: Round::Nil,
        proposer: Address::new([7; 20]),
        value: Value::new(0xABCD),
        validity: Validity::Valid,
    };

    // Encoded under ValueOrigin::Sync, but the origin is intentionally discarded on disk;
    // decode always returns ValueOrigin::Consensus.
    let entry = Input::ProposedValue(pv.clone(), ValueOrigin::Sync);
    let bytes = encode_to_bytes(&entry);
    assert_eq!(
        bytes[0], TAG_PROPOSED_VALUE,
        "ProposedValue must encode under TAG_PROPOSED_VALUE"
    );

    let decoded = round_trip(&entry);
    let Input::ProposedValue(got_pv, got_origin) = decoded else {
        panic!("expected ProposedValue variant, got {decoded:?}");
    };
    assert_eq!(got_pv, pv);
    assert!(
        got_origin.is_consensus(),
        "decoded ValueOrigin must default to Consensus regardless of encoded origin"
    );
}

#[test]
fn polka_certificate_round_trip() {
    let cert = PolkaCertificate {
        height: Height::new(7),
        round: Round::new(3),
        value_id: ValueId::new(0xC0FFEE),
        polka_signatures: vec![
            PolkaSignature::new(Address::new([1; 20]), Signature::test()),
            PolkaSignature::new(Address::new([2; 20]), Signature::test()),
        ],
    };

    let entry = Input::PolkaCertificate(cert.clone());
    let bytes = encode_to_bytes(&entry);
    assert_eq!(
        bytes[0], TAG_POLKA_CERTIFICATE,
        "PolkaCertificate must encode under TAG_POLKA_CERTIFICATE"
    );

    let decoded = round_trip(&entry);
    let Input::PolkaCertificate(got) = decoded else {
        panic!("expected PolkaCertificate variant, got {decoded:?}");
    };
    assert_eq!(got.height, cert.height);
    assert_eq!(got.round, cert.round);
    assert_eq!(got.value_id, cert.value_id);
    assert_eq!(got.polka_signatures.len(), cert.polka_signatures.len());
    for (got_sig, want_sig) in got
        .polka_signatures
        .iter()
        .zip(cert.polka_signatures.iter())
    {
        assert_eq!(got_sig.address, want_sig.address);
        assert_eq!(got_sig.signature.to_bytes(), want_sig.signature.to_bytes());
    }
}

/// A buffer with an invalid leading tag byte must surface as a decode error
/// rather than panicking or returning a bogus entry.
#[test]
fn invalid_tag_is_rejected() {
    let codec = ProtobufCodec;
    let buf = vec![0xFFu8]; // 0xFF is not assigned to any variant
    let result = decode_entry::<TestContext, _, _>(&codec, Cursor::new(buf));
    assert!(result.is_err(), "decode_entry should reject unknown tags");
}

/// `Input::Vote` and `Input::Proposal` share the same on-disk tag because both encode
/// through `SignedConsensusMsg<Ctx>`. This is the property that guarantees both variants
/// produce byte-compatible WAL files.
#[test]
fn vote_and_proposal_share_consensus_tag() {
    let vote = SignedVote::new(
        Vote::new_prevote(
            Height::new(1),
            Round::new(0),
            NilOrVal::Nil,
            Address::new([0; 20]),
        ),
        Signature::test(),
    );
    let proposal = SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            Value::new(0),
            Round::Nil,
            Address::new([0; 20]),
        ),
        Signature::test(),
    );

    let vote_bytes = encode_to_bytes(&Input::Vote(vote));
    let proposal_bytes = encode_to_bytes(&Input::Proposal(proposal));
    assert_eq!(vote_bytes[0], TAG_CONSENSUS);
    assert_eq!(proposal_bytes[0], TAG_CONSENSUS);
}

/// Hand-rolled byte sequence representing a `Prevote` timeout for round 5 on the
/// timeout-specific framing (`[tag][step][round: i64 BE]`). This pins the on-disk format down
/// independently of the round-trip tests: a regression that re-orders fields, changes the step
/// numbering, or alters the round encoding will fail this assertion even if encode/decode
/// stay self-consistent.
#[test]
fn timeout_decodes_fixed_byte_fixture() {
    let codec = ProtobufCodec;
    #[rustfmt::skip]
    let bytes: &[u8] = &[
        0x02,                                              // tag = TAG_TIMEOUT
        0x02,                                              // step = 2 (Prevote)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,    // round = 5 (i64 BE)
    ];
    let decoded = decode_entry::<TestContext, _, _>(&codec, Cursor::new(bytes.to_vec()))
        .expect("decode_entry on hand-rolled timeout bytes");
    let Input::TimeoutElapsed(got) = decoded else {
        panic!("expected TimeoutElapsed variant, got {decoded:?}");
    };
    assert_eq!(got.round, Round::new(5));
    assert_eq!(got.kind, malachitebft_core_types::TimeoutKind::Prevote);
}

/// The WAL codec must refuse [`Input`] variants that should not be persisted, so a stray
/// `Effect::WalAppend(_, Input::Propose(...), _)` (or similar) surfaces as a loud,
/// observable error instead of being silently dropped or producing a bogus entry. All four
/// non-persistable variants share the same encoder match arm, so exercising one is
/// representative.
#[test]
fn non_persistable_variant_is_rejected_with_invalid_input() {
    let codec = ProtobufCodec;

    let entry = Input::<TestContext>::Propose(LocallyProposedValue::new(
        Height::new(1),
        Round::new(0),
        Value::new(0),
    ));

    let mut buf = Vec::new();
    let err = encode_entry(entry, &codec, &mut buf)
        .expect_err("non-persistable variant must be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(buf.is_empty(), "rejection must not produce partial output");
}

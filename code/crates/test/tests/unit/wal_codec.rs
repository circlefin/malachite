//! Round-trip tests for the WAL frame codec (`engine::wal::{encode_entry, decode_entry}`).
//!
//! These tests cover the tag-byte + u64-length-prefix + codec-bytes framing
//! that `engine/src/wal/entry.rs` wraps around each `WalEntry` variant before
//! it is appended to the on-disk WAL. The framing is shared by all variants,
//! so a regression in any one variant's tag dispatch, length prefix, or codec
//! call is caught here without needing a multi-node end-to-end test.

use std::io::Cursor;

use arc_malachitebft_test::codec::proto::ProtobufCodec;
use arc_malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Value, ValueId, Vote,
};

use malachitebft_core_consensus::{ProposedValue, SignedConsensusMsg, WalEntry};
use malachitebft_core_types::{
    NilOrVal, PolkaCertificate, PolkaSignature, Round, SignedProposal, SignedVote, Timeout,
    Validity,
};
use malachitebft_engine::wal::{decode_entry, encode_entry};

fn round_trip(entry: &WalEntry<TestContext>) -> WalEntry<TestContext> {
    let codec = ProtobufCodec;
    let mut buf = Vec::new();
    encode_entry(entry, &codec, &mut buf).expect("encode_entry");
    decode_entry(&codec, Cursor::new(buf)).expect("decode_entry")
}

#[test]
fn wal_entry_consensus_msg_vote_round_trip() {
    let vote = SignedVote::new(
        Vote::new_prevote(
            Height::new(11),
            Round::new(2),
            NilOrVal::Val(ValueId::new(0xDEAD)),
            Address::new([9; 20]),
        ),
        Signature::test(),
    );

    let entry = WalEntry::ConsensusMsg(SignedConsensusMsg::Vote(vote.clone()));
    let decoded = round_trip(&entry);

    let WalEntry::ConsensusMsg(SignedConsensusMsg::Vote(got)) = decoded else {
        panic!("expected ConsensusMsg::Vote variant, got {decoded:?}");
    };
    assert_eq!(got.message, vote.message);
    assert_eq!(got.signature.to_bytes(), vote.signature.to_bytes());
}

#[test]
fn wal_entry_consensus_msg_proposal_round_trip() {
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

    let entry = WalEntry::ConsensusMsg(SignedConsensusMsg::Proposal(proposal.clone()));
    let decoded = round_trip(&entry);

    let WalEntry::ConsensusMsg(SignedConsensusMsg::Proposal(got)) = decoded else {
        panic!("expected ConsensusMsg::Proposal variant, got {decoded:?}");
    };
    assert_eq!(got.message, proposal.message);
    assert_eq!(got.signature.to_bytes(), proposal.signature.to_bytes());
}

#[test]
fn wal_entry_timeout_round_trip() {
    for timeout in [
        Timeout::propose(Round::new(0)),
        Timeout::prevote(Round::new(1)),
        Timeout::precommit(Round::new(2)),
        Timeout::rebroadcast(Round::new(3)),
    ] {
        let entry = WalEntry::Timeout(timeout);
        let decoded = round_trip(&entry);
        let WalEntry::Timeout(got) = decoded else {
            panic!("expected Timeout variant, got {decoded:?}");
        };
        assert_eq!(got.kind, timeout.kind);
        assert_eq!(got.round, timeout.round);
    }
}

#[test]
fn wal_entry_proposed_value_round_trip() {
    let pv = ProposedValue::<TestContext> {
        height: Height::new(42),
        round: Round::new(0),
        valid_round: Round::Nil,
        proposer: Address::new([7; 20]),
        value: Value::new(0xABCD),
        validity: Validity::Valid,
    };

    let entry = WalEntry::ProposedValue(pv.clone());
    let decoded = round_trip(&entry);

    let WalEntry::ProposedValue(got) = decoded else {
        panic!("expected ProposedValue variant, got {decoded:?}");
    };
    assert_eq!(got, pv);
}

#[test]
fn wal_entry_polka_certificate_round_trip() {
    let cert = PolkaCertificate {
        height: Height::new(7),
        round: Round::new(3),
        value_id: ValueId::new(0xC0FFEE),
        polka_signatures: vec![
            PolkaSignature::new(Address::new([1; 20]), Signature::test()),
            PolkaSignature::new(Address::new([2; 20]), Signature::test()),
        ],
    };

    let entry = WalEntry::PolkaCertificate(cert.clone());
    let decoded = round_trip(&entry);

    let WalEntry::PolkaCertificate(got) = decoded else {
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
fn wal_entry_invalid_tag_is_rejected() {
    let codec = ProtobufCodec;
    let buf = vec![0xFFu8]; // 0xFF is not assigned to any variant
    let result = decode_entry::<TestContext, _, _>(&codec, Cursor::new(buf));
    assert!(result.is_err(), "decode_entry should reject unknown tags");
}

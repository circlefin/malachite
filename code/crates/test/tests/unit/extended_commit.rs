use bytes::Bytes;
use futures::executor::block_on;

use arc_malachitebft_test::utils::validators::make_validators_seeded;
use arc_malachitebft_test::{Address, Ed25519Signer, Height, TestContext, ValidatorSet, ValueId};
use malachitebft_core_types::{
    CertificateError, Context, ExtendedCommitCertificate, ExtendedCommitSignature, NilOrVal, Round,
    SignedVote, ThresholdParams, Vote, VoteExtensionPolicy, VoteExtensionScope,
};
use malachitebft_signing::{Signer, VerifierExt};

const HEIGHT: u64 = 7;
const VALUE_ID: u64 = 42;

fn setup<const N: usize>(
    voting_powers: [u64; N],
    seed: u64,
) -> ([Address; N], [Ed25519Signer; N], ValidatorSet) {
    let pairs = make_validators_seeded(voting_powers, seed);
    let validators: Vec<_> = pairs.iter().map(|(v, _)| v.clone()).collect();
    let validator_set = ValidatorSet::new(validators.clone());

    let mut addresses = [Address::new([0u8; 20]); N];
    let mut signers: Vec<Ed25519Signer> = Vec::with_capacity(N);
    for (i, (validator, pk)) in pairs.into_iter().enumerate() {
        addresses[i] = validator.address;
        signers.push(Ed25519Signer::new(pk));
    }

    (addresses, signers.try_into().unwrap(), validator_set)
}

fn signed_precommit_with_extension(
    ctx: &TestContext,
    signer: &Ed25519Signer,
    address: Address,
    extension: Option<Bytes>,
) -> SignedVote<TestContext> {
    let vote = ctx.new_precommit(
        Height::new(HEIGHT),
        Round::new(0),
        NilOrVal::Val(ValueId::new(VALUE_ID)),
        address,
    );

    let mut signed = block_on(signer.sign_vote(vote)).unwrap();

    if let Some(ext) = extension {
        let scope = VoteExtensionScope::new(
            Height::new(HEIGHT),
            Round::new(0),
            ValueId::new(VALUE_ID),
            address,
        );
        let signed_ext = block_on(signer.sign_vote_extension(scope, ext)).unwrap();
        signed.message = signed.message.extend(signed_ext);
    }

    signed
}

#[test]
fn verify_succeeds_when_every_precommit_carries_a_bound_extension() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA0);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ))
    .expect("certificate with bound extensions must verify");
}

#[test]
fn verify_rejects_when_no_precommit_carries_an_extension() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA1);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, None),
        signed_precommit_with_extension(&ctx, &s1, a1, None),
        signed_precommit_with_extension(&ctx, &s2, a2, None),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    assert!(cert.commit_signatures.iter().all(|s| s.extension.is_none()));

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::MissingVoteExtension(_))
    ));
}

#[test]
fn verify_accepts_missing_extensions_when_policy_is_disabled() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA1);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, None),
        signed_precommit_with_extension(&ctx, &s1, a1, None),
        signed_precommit_with_extension(&ctx, &s2, a2, None),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Disabled,
    ))
    .expect("missing extensions are accepted when the policy is disabled");
}

#[test]
fn verify_rejects_present_extension_when_policy_is_disabled() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA2);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, None),
        signed_precommit_with_extension(&ctx, &s2, a2, None),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Disabled,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::UnexpectedVoteExtension(addr)) if addr == a0
    ));
}

#[test]
fn verify_rejects_when_any_precommit_lacks_an_extension() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA2);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, None),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::MissingVoteExtension(addr)) if addr == a1
    ));
}

#[test]
fn verify_rejects_extension_swapped_between_validators() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA3);

    let mut votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    // Swap validator 0's extension onto validator 1's vote. The extension was
    // signed by validator 0 for scope (h, r, v, addr_0), so verifying it
    // against (h, r, v, addr_1) with validator 1's public key must fail.
    let stolen = votes[0].message.extension().cloned().unwrap();
    votes[1].message = votes[1].message.clone().extend(stolen);

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::InvalidVoteExtensionSignature(_))
    ));
}

#[test]
fn verify_rejects_extension_signed_for_different_height() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA4);

    // Validator 0 produces a vote for HEIGHT but an extension signed against HEIGHT+1.
    let bogus_scope = VoteExtensionScope::new(
        Height::new(HEIGHT + 1),
        Round::new(0),
        ValueId::new(VALUE_ID),
        a0,
    );
    let bogus_ext =
        block_on(s0.sign_vote_extension(bogus_scope, Bytes::from_static(b"replay"))).unwrap();

    let mut vote0 = signed_precommit_with_extension(&ctx, &s0, a0, None);
    vote0.message = vote0.message.extend(bogus_ext);

    let votes = vec![
        vote0,
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::InvalidVoteExtensionSignature(addr)) if addr == a0
    ));
}

#[test]
fn verify_rejects_extension_signed_for_different_round() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xAD);

    // Validator 0 produces a vote for round 0 but an extension signed against round 1.
    let bogus_scope = VoteExtensionScope::new(
        Height::new(HEIGHT),
        Round::new(1),
        ValueId::new(VALUE_ID),
        a0,
    );
    let bogus_ext =
        block_on(s0.sign_vote_extension(bogus_scope, Bytes::from_static(b"replay"))).unwrap();

    let mut vote0 = signed_precommit_with_extension(&ctx, &s0, a0, None);
    vote0.message = vote0.message.extend(bogus_ext);

    let votes = vec![
        vote0,
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::InvalidVoteExtensionSignature(addr)) if addr == a0
    ));
}

#[test]
fn verify_rejects_extension_signed_for_different_value_id() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xAE);

    // Validator 0 produces a vote for VALUE_ID but an extension signed against another value.
    let bogus_scope = VoteExtensionScope::new(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID + 1),
        a0,
    );
    let bogus_ext =
        block_on(s0.sign_vote_extension(bogus_scope, Bytes::from_static(b"replay"))).unwrap();

    let mut vote0 = signed_precommit_with_extension(&ctx, &s0, a0, None);
    vote0.message = vote0.message.extend(bogus_ext);

    let votes = vec![
        vote0,
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::InvalidVoteExtensionSignature(addr)) if addr == a0
    ));
}

#[test]
fn verify_rejects_tampered_commit_signature() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xA5);

    let mut votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    // Replace validator 0's precommit signature with validator 1's (signed over
    // validator 1's address). The reconstructed precommit for `a0` will fail
    // verification with this signature.
    votes[0].signature = votes[1].signature;

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::InvalidCommitSignature(_))
    ));
}

#[test]
fn verify_rejects_when_voting_power_below_quorum() {
    let ctx = TestContext::new();
    // 3 validators of equal weight; supply only one precommit (1/3 < 2/3).
    let ([a0, _, _], [s0, _, _], validator_set) = setup([1, 1, 1], 0xA6);

    let votes = vec![signed_precommit_with_extension(
        &ctx,
        &s0,
        a0,
        Some(Bytes::from_static(b"ext-0")),
    )];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::NotEnoughVotingPower { .. })
    ));
}

#[test]
fn verify_rejects_duplicate_validator_signature() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xAB);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0a"))),
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0b"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let result = block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &cert,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ));

    assert!(matches!(
        result,
        Err(CertificateError::DuplicateVote(addr)) if addr == a0
    ));
}

#[test]
fn projection_to_commit_certificate_drops_extensions_but_keeps_signatures() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], _) = setup([1, 1, 1], 0xA7);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, None),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let bare = cert.trim_vote_extensions();

    assert_eq!(bare.height, cert.height);
    assert_eq!(bare.round, cert.round);
    assert_eq!(bare.value_id, cert.value_id);
    assert_eq!(bare.commit_signatures.len(), cert.commit_signatures.len());
    for (b, e) in bare
        .commit_signatures
        .iter()
        .zip(cert.commit_signatures.iter())
    {
        assert_eq!(b.address, e.address);
        assert_eq!(b.signature, e.signature);
    }
}

#[test]
fn vote_extensions_view_carries_only_signed_extensions() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], _) = setup([1, 1, 1], 0xA8);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, None),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let cert = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let extensions = cert.vote_extensions();

    assert_eq!(extensions.extensions.len(), 2);
    let addrs: Vec<_> = extensions.extensions.iter().map(|(a, _)| *a).collect();
    assert!(addrs.contains(&a0));
    assert!(addrs.contains(&a2));
    assert!(!addrs.contains(&a1));
}

#[test]
fn from_commit_certificate_and_extensions_rebuilds_the_bundled_type_and_verifies() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], validator_set) = setup([1, 1, 1], 0xAB);

    // Build a fully-bundled certificate, then split it into the parallel
    // (CommitCertificate, VoteExtensions) shape that the host API exposes.
    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let bundled = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let bare = bundled.trim_vote_extensions();
    let extensions = bundled.vote_extensions();

    // An app holding only the parallel pair must be able to rebuild the
    // bundled type and have it pass the full extended-cert verification.
    let rebuilt =
        ExtendedCommitCertificate::from_commit_certificate_and_extensions(bare, extensions);

    assert_eq!(rebuilt.commit_signatures.len(), 3);
    let by_addr: std::collections::HashMap<_, _> = rebuilt
        .commit_signatures
        .iter()
        .map(|s| (s.address, s.extension.is_some()))
        .collect();
    assert!(by_addr[&a0]);
    assert!(by_addr[&a1]);
    assert!(by_addr[&a2]);

    block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &rebuilt,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ))
    .expect("rebuilt certificate must verify against the validator set");
}

#[test]
fn from_commit_certificate_and_extensions_discards_unmatched_extensions() {
    let ctx = TestContext::new();
    let ([a0, a1, a2, a3], [s0, s1, s2, s3], validator_set) = setup([1, 1, 1, 1], 0xAF);

    // Build a certificate with 3/4 validators, then provide an extra extension
    // for the fourth validator. The extra extension cannot be bound to any
    // commit signature, so the constructor should discard it.
    let votes = vec![
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
    ];

    let bundled = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let bare = bundled.trim_vote_extensions();
    let mut extensions = bundled.vote_extensions();
    let unmatched_scope = VoteExtensionScope::new(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        a3,
    );
    let unmatched_ext =
        block_on(s3.sign_vote_extension(unmatched_scope, Bytes::from_static(b"ext-3"))).unwrap();
    extensions.extensions.push((a3, unmatched_ext));

    let rebuilt =
        ExtendedCommitCertificate::from_commit_certificate_and_extensions(bare, extensions);

    assert_eq!(rebuilt.commit_signatures.len(), 3);
    assert!(rebuilt.commit_signatures.iter().all(|s| s.address != a3));
    assert!(rebuilt
        .commit_signatures
        .iter()
        .all(|s| s.extension.is_some()));

    block_on(s0.verify_extended_commit_certificate(
        &ctx,
        &rebuilt,
        &validator_set,
        ThresholdParams::default(),
        VoteExtensionPolicy::Required,
    ))
    .expect("rebuilt certificate must verify after discarding unmatched extensions");
}

#[test]
fn constructors_canonicalize_signature_order_by_address() {
    let ctx = TestContext::new();
    let ([a0, a1, a2], [s0, s1, s2], _) = setup([1, 1, 1], 0xAC);

    let votes = vec![
        signed_precommit_with_extension(&ctx, &s2, a2, Some(Bytes::from_static(b"ext-2"))),
        signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"ext-0"))),
        signed_precommit_with_extension(&ctx, &s1, a1, Some(Bytes::from_static(b"ext-1"))),
    ];

    let from_votes = ExtendedCommitCertificate::from_votes(
        Height::new(HEIGHT),
        Round::new(0),
        ValueId::new(VALUE_ID),
        votes,
    );

    let mut certificate = from_votes.trim_vote_extensions();
    certificate.commit_signatures.reverse();

    let mut extensions = from_votes.vote_extensions();
    extensions.extensions.reverse();

    let from_pair =
        ExtendedCommitCertificate::from_commit_certificate_and_extensions(certificate, extensions);

    let from_votes_addresses: Vec<_> = from_votes
        .commit_signatures
        .iter()
        .map(|s| s.address)
        .collect();
    let from_pair_addresses: Vec<_> = from_pair
        .commit_signatures
        .iter()
        .map(|s| s.address)
        .collect();
    let mut sorted_addresses = vec![a0, a1, a2];
    sorted_addresses.sort();

    assert_eq!(from_votes_addresses, sorted_addresses);
    assert_eq!(from_pair_addresses, sorted_addresses);
}

#[test]
fn extended_commit_signature_constructor_round_trips() {
    let ctx = TestContext::new();
    let ([a0, _, _], [s0, _, _], _) = setup([1, 1, 1], 0xAA);

    let signed = signed_precommit_with_extension(&ctx, &s0, a0, Some(Bytes::from_static(b"hello")));
    let extension = signed.message.extension().cloned();

    let sig = ExtendedCommitSignature::<TestContext>::new(a0, signed.signature, extension);

    assert_eq!(sig.address, a0);
    assert!(sig.extension.is_some());
}

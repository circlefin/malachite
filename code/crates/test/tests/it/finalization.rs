use rstest::rstest;
use std::time::Duration;

use malachitebft_config::ValuePayload;
use malachitebft_core_types::CommitCertificate;
use malachitebft_test_framework::{HandlerResult, TestParams};

use crate::{TestBuilder, TestContext};

fn validate_certificate(certificate: &CommitCertificate<TestContext>) {
    assert!(certificate.height.as_u64() > 0, "Should have valid height");
    assert!(
        !certificate.commit_signatures.is_empty(),
        "Should have signatures"
    );
    assert!(
        certificate.commit_signatures.len() >= 2,
        "Should have at least quorum signatures"
    );
}
#[derive(Default)]
struct SignatureCountTracker {
    decided_signatures: Option<usize>,
}

#[rstest]
#[case::no_target_time(None, ValuePayload::ProposalAndParts, false)]
#[case::target_time(Some(Duration::from_millis(10)), ValuePayload::ProposalAndParts, false)]
#[case::with_parts_only(Some(Duration::from_millis(1)), ValuePayload::PartsOnly, false)]
#[case::with_stable_block_times(
    Some(Duration::from_millis(5)),
    ValuePayload::ProposalAndParts,
    true
)]
#[case::with_both(Some(Duration::from_millis(15)), ValuePayload::PartsOnly, true)]
#[tokio::test]
pub async fn test_finalize_with_params(
    #[case] target_time: Option<Duration>,
    #[case] value_payload: ValuePayload,
    #[case] stable_block_times: bool,
) {
    const HEIGHT: u64 = 4;

    let mut test = TestBuilder::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node()
        .start()
        .with_state(SignatureCountTracker::default())
        .on_decided(|certificate, state| {
            validate_certificate(&certificate);
            assert!(state.decided_signatures.is_none(), "No sigs before decided");
            state.decided_signatures = Some(certificate.commit_signatures.len());

            Ok(HandlerResult::ContinueTest)
        })
        .on_finalized(|certificate, _evidence, state| {
            validate_certificate(&certificate);
            let decided_sigs = state.decided_signatures.take().unwrap();
            assert!(
                certificate.commit_signatures.len() >= decided_sigs,
                "Finalized certificate shouldn't have less signatures",
            );

            Ok(HandlerResult::ContinueTest)
        })
        .success();
    test.add_node()
        .start()
        .with_state(SignatureCountTracker::default())
        .on_decided(|certificate, state| {
            validate_certificate(&certificate);
            assert!(state.decided_signatures.is_none(), "No sigs before decided");
            state.decided_signatures = Some(certificate.commit_signatures.len());

            Ok(HandlerResult::ContinueTest)
        })
        .on_finalized(|certificate, _evidence, state| {
            validate_certificate(&certificate);
            let decided_sigs = state.decided_signatures.take().unwrap();
            assert!(
                certificate.commit_signatures.len() >= decided_sigs,
                "Finalized certificate shouldn't have less signatures",
            );

            Ok(HandlerResult::ContinueTest)
        })
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(10),
            TestParams {
                target_time,
                value_payload,
                stable_block_times,
                ..TestParams::default()
            },
        )
        .await
}

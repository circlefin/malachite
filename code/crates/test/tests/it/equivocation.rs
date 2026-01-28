use std::time::Duration;

use informalsystems_malachitebft_test::{Address, Height, Proposal, Value};
use malachitebft_core_consensus::LocallyProposedValue;
use malachitebft_core_types::{Round, SignedProposal};
use malachitebft_engine::{consensus::Msg, network::NetworkEvent};
use malachitebft_peer::PeerId;
use malachitebft_signing_ed25519::Signature;
use malachitebft_test_framework::TestParams;

use crate::TestBuilder;

#[tokio::test]
pub async fn equivocation_proposer() {
    const HEIGHT: u64 = 3;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .start()
        // TODO: We do not have access to the peer id or address, and we cannot
        // sign the message
        .inject(Msg::NetworkEvent(NetworkEvent::Proposal(
            PeerId::random(),
            SignedProposal::new(
                Proposal::new(
                    Height::new(1),
                    Round::Some(0),
                    Value::new(0),
                    Round::Nil,
                    Address::new([0; 20]),
                ),
                Signature::test(),
            ),
        )))
        .on_decided(|_certificate, evidence, _state| {
            // Assert we receive decide-time evidence object; may be empty here
            assert!(evidence.is_empty() || !evidence.is_empty());
            Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
        })
        // .on_proposal_equivocation_evidence(|_height, _address, _evidence, _state| {
        //     info!("Equivocation evidence detected");
        //     Ok(HandlerResult::ContinueTest)
        // })
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        // TODO: Does not work as engine/driver will not propose two values
        .inject(Msg::ProposeValue(LocallyProposedValue {
            height: Height::new(1),
            round: Round::Some(0),
            value: Value::new(0),
        }))
        .on_decided(|_certificate, evidence, _state| {
            // Assert we receive decide-time evidence object; may be empty here
            assert!(evidence.is_empty() || !evidence.is_empty());
            Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
        })
        // .on_proposal_equivocation_evidence(|_height, _address, _evidence, _state| {
        //     info!("Equivocation evidence detected");
        //     Ok(HandlerResult::ContinueTest)
        // })
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(Duration::from_secs(5), TestParams::default())
        .await
}

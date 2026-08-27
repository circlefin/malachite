use std::time::Duration;

use bytesize::ByteSize;
use eyre::bail;

use malachitebft_core_types::{NilOrVal, Vote as _, VoteType};
use malachitebft_test_framework::{HandlerResult, TestParams};

use crate::TestBuilder;

#[tokio::test]
pub async fn vote_extensions_are_signed_and_verified_end_to_end() {
    const HEIGHT: u64 = 3;
    const EXTENSION_SIZE: u64 = 32;

    let mut test = TestBuilder::<()>::new();

    for _ in 0..3 {
        test.add_node()
            .start()
            .on_vote(|vote, _| {
                if vote.vote_type() != VoteType::Precommit {
                    return Ok(HandlerResult::WaitForNextEvent);
                }

                let NilOrVal::Val(_) = vote.value() else {
                    // Nil precommits are legitimate and don't carry extensions.
                    return Ok(HandlerResult::WaitForNextEvent);
                };

                let Some(extension) = vote.extension() else {
                    bail!("published precommit without vote extension");
                };

                let expected_prefix = format!("ext h={} r={}", vote.height(), vote.round());
                if !extension.message.starts_with(expected_prefix.as_bytes()) {
                    bail!(
                        "unexpected vote-extension payload {:?}, expected prefix {expected_prefix:?}",
                        extension.message
                    );
                }

                if extension.message.len() != EXTENSION_SIZE as usize {
                    bail!(
                        "unexpected vote-extension size {}, expected {EXTENSION_SIZE}",
                        extension.message.len()
                    );
                }

                Ok(HandlerResult::ContinueTest)
            })
            .wait_until(HEIGHT)
            .success();
    }

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams::default().enable_vote_extensions(ByteSize::b(EXTENSION_SIZE)),
        )
        .await
}

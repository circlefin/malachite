use std::time::Duration;

use informalsystems_malachitebft_test::middleware::RotateValidators;

use crate::TestBuilder;

#[tokio::test]
async fn rotate_validator_set() {
    const HEIGHT: u64 = 20;
    const NUM_NODES: usize = 5;
    const NUM_VALIDATORS_PER_HEIGHT: usize = 3;

    let mut test = TestBuilder::<()>::new();

    for _ in 0..NUM_NODES {
        test.add_node()
            .with_middleware(RotateValidators {
                selection_size: NUM_VALIDATORS_PER_HEIGHT,
            })
            .start()
            .wait_until(HEIGHT)
            .success();
    }

    test.build().run(Duration::from_secs(50)).await
}

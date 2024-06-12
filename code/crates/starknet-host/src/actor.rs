use ractor::{async_trait, Actor, ActorProcessingErr};

use malachite_test::TestContext;

use crate::mock::host::MockHost;
use crate::mock::part_store::PartStore;

pub struct StarknetHost {
    host: MockHost,
}

pub struct HostState {
    part_store: PartStore<TestContext>,
}

pub type HostRef = malachite_actors::host::HostRef<TestContext>;
pub type HostMsg = malachite_actors::host::HostMsg<TestContext>;

#[async_trait]
impl Actor for StarknetHost {
    type Arguments = HostState;
    type State = HostState;
    type Msg = HostMsg;

    async fn pre_start(
        &self,
        _myself: HostRef,
        initial_state: Self::State,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(initial_state)
    }

    async fn handle(
        &self,
        _myself: HostRef,
        _msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }
}

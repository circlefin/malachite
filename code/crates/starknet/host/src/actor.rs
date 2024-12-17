use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use eyre::eyre;
use itertools::Itertools;
use ractor::{async_trait, Actor, ActorProcessingErr, RpcReplyPort, SpawnErr};
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::time::Instant;
use tracing::{debug, error, info, trace, warn};

use malachite_consensus::PeerId;
use malachite_core_types::{CommitCertificate, Round, Validity, ValueOrigin};
use malachite_engine::consensus::{ConsensusMsg, ConsensusRef};
use malachite_engine::host::{LocallyProposedValue, ProposedValue};
use malachite_engine::network::{NetworkMsg, NetworkRef};
use malachite_engine::util::streaming::{StreamContent, StreamMessage};
use malachite_metrics::Metrics;
use malachite_sync::DecidedValue;

use crate::host::proposal::compute_proposal_signature;
use crate::host::state::HostState;
use crate::host::{Host as _, StarknetHost};
use crate::mempool::{MempoolMsg, MempoolRef};
use crate::proto::Protobuf;
use crate::types::*;

pub struct Host {
    mempool: MempoolRef,
    network: NetworkRef<MockContext>,
    metrics: Metrics,
    span: tracing::Span,
}

pub type HostRef = malachite_engine::host::HostRef<MockContext>;
pub type HostMsg = malachite_engine::host::HostMsg<MockContext>;

impl Host {
    pub async fn spawn(
        home_dir: PathBuf,
        host: StarknetHost,
        mempool: MempoolRef,
        network: NetworkRef<MockContext>,
        metrics: Metrics,
        span: tracing::Span,
    ) -> Result<HostRef, SpawnErr> {
        let db_dir = home_dir.join("db");
        std::fs::create_dir_all(&db_dir).map_err(|e| SpawnErr::StartupFailed(e.into()))?;
        let db_path = db_dir.join("blocks.db");

        let (actor_ref, _) = Actor::spawn(
            None,
            Self::new(mempool, network, metrics, span),
            HostState::new(host, db_path, &mut StdRng::from_entropy()),
        )
        .await?;

        Ok(actor_ref)
    }

    pub fn new(
        mempool: MempoolRef,
        network: NetworkRef<MockContext>,
        metrics: Metrics,
        span: tracing::Span,
    ) -> Self {
        Self {
            mempool,
            network,
            metrics,
            span,
        }
    }
}

#[async_trait]
impl Actor for Host {
    type Arguments = HostState;
    type State = HostState;
    type Msg = HostMsg;

    async fn pre_start(
        &self,
        myself: HostRef,
        initial_state: Self::State,
    ) -> Result<Self::State, ActorProcessingErr> {
        self.mempool.link(myself.get_cell());

        Ok(initial_state)
    }

    async fn handle(
        &self,
        _myself: HostRef,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let Err(e) = self.handle_msg(_myself, msg, state).await {
            error!(%e, "Failed to handle message");
        }

        Ok(())
    }
}

impl Host {
    #[tracing::instrument(
        name = "host",
        parent = &self.span,
        skip_all,
        fields(height = %state.height, round = %state.round),
    )]
    async fn handle_msg(
        &self,
        _myself: HostRef,
        msg: HostMsg,
        state: &mut HostState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            HostMsg::ConsensusReady(consensus) => on_consensus_ready(state, consensus),

            HostMsg::StartedRound {
                height,
                round,
                proposer,
            } => on_started_round(state, height, round, proposer).await,

            HostMsg::GetHistoryMinHeight { reply_to } => on_get_history_min_height(state, reply_to),

            HostMsg::GetValue {
                height,
                round,
                timeout,
                reply_to,
            } => on_get_value(state, &self.network, height, round, timeout, reply_to).await,

            HostMsg::RestreamValue {
                height,
                round,
                valid_round,
                address,
                value_id,
            } => {
                on_restream_value(
                    state,
                    &self.network,
                    height,
                    round,
                    value_id,
                    valid_round,
                    address,
                )
                .await
            }

            HostMsg::ReceivedProposalPart {
                from,
                part,
                reply_to,
            } => on_received_proposal_part(state, part, from, reply_to).await,

            HostMsg::GetValidatorSet { height, reply_to } => {
                on_get_validator_set(state, height, reply_to).await
            }

            HostMsg::Decided {
                certificate,
                consensus,
            } => on_decided(state, &consensus, &self.mempool, certificate, &self.metrics).await,

            HostMsg::GetDecidedValue { height, reply_to } => {
                on_get_decided_block(height, state, reply_to).await
            }

            HostMsg::ProcessSyncedValue {
                height,
                round,
                validator_address,
                value_bytes,
                reply_to,
            } => on_process_synced_value(value_bytes, height, round, validator_address, reply_to),
        }
    }
}

fn on_consensus_ready(
    state: &mut HostState,
    consensus: ConsensusRef<MockContext>,
) -> Result<(), ActorProcessingErr> {
    let latest_block_height = state.block_store.last_height().unwrap_or_default();
    let start_height = latest_block_height.increment();

    state.consensus = Some(consensus.clone());

    consensus.cast(ConsensusMsg::StartHeight(
        start_height,
        state.host.validator_set.clone(),
    ))?;

    Ok(())
}

async fn replay_undecided_values(
    state: &mut HostState,
    height: Height,
    round: Round,
) -> Result<(), ActorProcessingErr> {
    let undecided_values = state
        .block_store
        .get_undecided_values(height, round)
        .await?;

    let consensus = state.consensus.as_ref().unwrap();

    for value in undecided_values {
        info!(%height, %round, hash = %value.value, "Replaying already known proposed value");

        consensus.cast(ConsensusMsg::ReceivedProposedValue(
            value,
            ValueOrigin::Consensus,
        ))?;
    }

    Ok(())
}

async fn on_started_round(
    state: &mut HostState,
    height: Height,
    round: Round,
    proposer: Address,
) -> Result<(), ActorProcessingErr> {
    state.height = height;
    state.round = round;
    state.proposer = Some(proposer);

    // If we have already built or seen one or more values for this height and round,
    // feed them back to consensus. This may happen when we are restarting after a crash.
    replay_undecided_values(state, height, round).await?;

    Ok(())
}

fn on_get_history_min_height(
    state: &mut HostState,
    reply_to: RpcReplyPort<Height>,
) -> Result<(), ActorProcessingErr> {
    let history_min_height = state.block_store.first_height().unwrap_or_default();
    reply_to.send(history_min_height)?;

    Ok(())
}

async fn on_get_validator_set(
    state: &mut HostState,
    height: Height,
    reply_to: RpcReplyPort<ValidatorSet>,
) -> Result<(), ActorProcessingErr> {
    let Some(validators) = state.host.validators(height).await else {
        return Err(eyre!("No validator set found for the given height {height}").into());
    };

    reply_to.send(ValidatorSet::new(validators))?;
    Ok(())
}

async fn on_get_value(
    state: &mut HostState,
    network: &NetworkRef<MockContext>,
    height: Height,
    round: Round,
    timeout: Duration,
    reply_to: RpcReplyPort<LocallyProposedValue<MockContext>>,
) -> Result<(), ActorProcessingErr> {
    if let Some(value) = find_previously_built_value(state, height, round).await? {
        info!(%height, %round, hash = %value.value, "Returning previously built value");

        reply_to.send(LocallyProposedValue::new(
            value.height,
            value.round,
            value.value,
            value.extension,
        ))?;

        return Ok(());
    }

    let deadline = Instant::now() + timeout;

    debug!(%height, %round, "Building new proposal...");

    let (mut rx_part, rx_hash) = state.host.build_new_proposal(height, round, deadline).await;

    let stream_id = state.next_stream_id();

    let mut sequence = 0;

    while let Some(part) = rx_part.recv().await {
        state.host.part_store.store(height, round, part.clone());

        if state.host.params.value_payload.include_parts() {
            debug!(%stream_id, %sequence, "Broadcasting proposal part");

            let msg = StreamMessage::new(stream_id, sequence, StreamContent::Data(part.clone()));
            network.cast(NetworkMsg::PublishProposalPart(msg))?;
        }

        sequence += 1;
    }

    if state.host.params.value_payload.include_parts() {
        let msg = StreamMessage::new(stream_id, sequence, StreamContent::Fin(true));
        network.cast(NetworkMsg::PublishProposalPart(msg))?;
    }

    let block_hash = rx_hash.await?;
    debug!(%block_hash, "Assembled block");

    state
        .host
        .part_store
        .store_value_id(height, round, block_hash);

    let parts = state.host.part_store.all_parts(height, round);

    let Some(value) = state.build_value_from_parts(&parts, height, round).await else {
        error!(%height, %round, "Failed to build block from parts");
        return Ok(());
    };

    debug!(%height, %round, %block_hash, "Storing proposed value from assembled block");
    if let Err(e) = state.block_store.store_undecided_value(value.clone()).await {
        error!(%e, %height, %round, "Failed to store the proposed value");
    }

    reply_to.send(LocallyProposedValue::new(
        value.height,
        value.round,
        value.value,
        value.extension,
    ))?;

    Ok(())
}

/// If we have already built a block for this height and round, return it to consensus
/// This may happen when we are restarting after a crash and replaying the WAL.
async fn find_previously_built_value(
    state: &mut HostState,
    height: Height,
    round: Round,
) -> Result<Option<ProposedValue<MockContext>>, ActorProcessingErr> {
    let values = state
        .block_store
        .get_undecided_values(height, round)
        .await?;

    let proposed_value = values
        .into_iter()
        .find(|v| v.validator_address == state.host.address);

    Ok(proposed_value)
}

async fn on_restream_value(
    state: &mut HostState,
    network: &NetworkRef<MockContext>,
    height: Height,
    round: Round,
    value_id: Hash,
    valid_round: Round,
    address: Address,
) -> Result<(), ActorProcessingErr> {
    debug!(%height, %round, "Restreaming existing proposal...");

    let mut rx_part = state.host.send_known_proposal(value_id).await;

    let stream_id = state.next_stream_id();

    let init = ProposalInit {
        height,
        proposal_round: round,
        valid_round,
        proposer: address,
    };

    let signature = compute_proposal_signature(&init, &value_id, &state.host.private_key);

    let init_part = ProposalPart::Init(init);
    let fin_part = ProposalPart::Fin(ProposalFin { signature });

    debug!(%height, %round, "Created new Init part: {init_part:?}");

    let mut sequence = 0;

    while let Some(part) = rx_part.recv().await {
        let new_part = match part.part_type() {
            PartType::Init => init_part.clone(),
            PartType::Fin => fin_part.clone(),
            PartType::Transactions | PartType::BlockProof => part,
        };

        state.host.part_store.store(height, round, new_part.clone());

        if state.host.params.value_payload.include_parts() {
            debug!(%stream_id, %sequence, "Broadcasting proposal part");

            let msg = StreamMessage::new(stream_id, sequence, StreamContent::Data(new_part));

            network.cast(NetworkMsg::PublishProposalPart(msg))?;

            sequence += 1;
        }
    }

    Ok(())
}

fn on_process_synced_value(
    value_bytes: Bytes,
    height: Height,
    round: Round,
    validator_address: Address,
    reply_to: RpcReplyPort<ProposedValue<MockContext>>,
) -> Result<(), ActorProcessingErr> {
    let maybe_block = Block::from_bytes(value_bytes.as_ref());
    if let Ok(block) = maybe_block {
        let proposed_value = ProposedValue {
            height,
            round,
            valid_round: Round::Nil,
            validator_address,
            value: block.block_hash,
            validity: Validity::Valid,
            extension: None,
        };

        reply_to.send(proposed_value)?;
    }

    Ok(())
}

async fn on_get_decided_block(
    height: Height,
    state: &mut HostState,
    reply_to: RpcReplyPort<Option<DecidedValue<MockContext>>>,
) -> Result<(), ActorProcessingErr> {
    debug!(%height, "Received request for block");

    match state.block_store.get(height).await {
        Ok(None) => {
            let min = state.block_store.first_height().unwrap_or_default();
            let max = state.block_store.last_height().unwrap_or_default();

            warn!(%height, "No block for this height, available blocks: {min}..={max}");

            reply_to.send(None)?;
        }

        Ok(Some(block)) => {
            let block = DecidedValue {
                value_bytes: block.block.to_bytes().unwrap(),
                certificate: block.certificate,
            };

            debug!(%height, "Found decided block in store");
            reply_to.send(Some(block))?;
        }
        Err(e) => {
            error!(%e, %height, "Failed to get decided block");
            reply_to.send(None)?;
        }
    }

    Ok(())
}

async fn on_received_proposal_part(
    state: &mut HostState,
    part: StreamMessage<ProposalPart>,
    from: PeerId,
    reply_to: RpcReplyPort<ProposedValue<MockContext>>,
) -> Result<(), ActorProcessingErr> {
    // TODO - use state.host.receive_proposal() and move some of the logic below there
    let sequence = part.sequence;

    let Some(parts) = state.part_streams_map.insert(from, part) else {
        return Ok(());
    };

    if parts.height < state.height {
        trace!(
            height = %state.height,
            round = %state.round,
            part.height = %parts.height,
            part.round = %parts.round,
            part.sequence = %sequence,
            "Received outdated proposal part, ignoring"
        );

        return Ok(());
    }

    for part in parts.parts {
        debug!(
            part.sequence = %sequence,
            part.height = %parts.height,
            part.round = %parts.round,
            part.message = ?part.part_type(),
            "Processing proposal part"
        );

        if let Some(value) = state
            .build_value_from_part(parts.height, parts.round, part)
            .await
        {
            debug!(
                height = %value.height, round = %value.round, block_hash = %value.value,
                "Storing proposed value assembled from proposal parts"
            );

            if let Err(e) = state.block_store.store_undecided_value(value.clone()).await {
                error!(
                    %e, height = %value.height, round = %value.round, block_hash = %value.value,
                    "Failed to store the proposed value"
                );
            }

            reply_to.send(value)?;
            break;
        }
    }

    Ok(())
}

async fn on_decided(
    state: &mut HostState,
    consensus: &ConsensusRef<MockContext>,
    mempool: &MempoolRef,
    certificate: CommitCertificate<MockContext>,
    metrics: &Metrics,
) -> Result<(), ActorProcessingErr> {
    let (height, round) = (certificate.height, certificate.round);

    let mut all_parts = state.host.part_store.all_parts(height, round);

    let mut all_txes = vec![];
    for part in all_parts.iter_mut() {
        if let ProposalPart::Transactions(transactions) = part.as_ref() {
            let mut txes = transactions.to_vec();
            all_txes.append(&mut txes);
        }
    }

    // Build the block from transaction parts and certificate, and store it
    if let Err(e) = state
        .block_store
        .store_decided_block(&certificate, &all_txes)
        .await
    {
        error!(%e, %height, %round, "Failed to store the block");
    }

    // Update metrics
    let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
    let extension_size: usize = certificate
        .aggregated_signature
        .signatures
        .iter()
        .map(|c| c.extension.as_ref().map(|e| e.size_bytes()).unwrap_or(0))
        .sum();

    let block_and_commits_size = block_size + extension_size;
    let tx_count: usize = all_parts.iter().map(|p| p.tx_count()).sum();

    metrics.block_tx_count.observe(tx_count as f64);
    metrics
        .block_size_bytes
        .observe(block_and_commits_size as f64);
    metrics.finalized_txes.inc_by(tx_count as u64);

    // Gather hashes of all the tx-es included in the block,
    // so that we can notify the mempool to remove them.
    let mut tx_hashes = vec![];
    for part in all_parts {
        if let ProposalPart::Transactions(txes) = &part.as_ref() {
            tx_hashes.extend(txes.as_slice().iter().map(|tx| tx.hash()));
        }
    }

    // Prune the PartStore of all parts for heights lower than `state.height`
    state.host.part_store.prune(state.height);

    // Store the block
    prune_block_store(state).await;

    // Notify the mempool to remove corresponding txs
    mempool.cast(MempoolMsg::Update { tx_hashes })?;

    // Notify Starknet Host of the decision
    state.host.decision(certificate).await;

    // Start the next height
    consensus.cast(ConsensusMsg::StartHeight(
        state.height.increment(),
        state.host.validator_set.clone(),
    ))?;

    Ok(())
}

async fn prune_block_store(state: &mut HostState) {
    let max_height = state.block_store.last_height().unwrap_or_default();
    let max_retain_blocks = state.host.params.max_retain_blocks as u64;

    // Compute the height to retain blocks higher than
    let retain_height = max_height.as_u64().saturating_sub(max_retain_blocks);
    if retain_height <= 1 {
        // No need to prune anything, since we would retain every blocks
        return;
    }

    let retain_height = Height::new(retain_height, max_height.fork_id);
    match state.block_store.prune(retain_height).await {
        Ok(pruned) => {
            debug!(
                %retain_height, pruned_heights = pruned.iter().join(", "),
                "Pruned the block store"
            );
        }
        Err(e) => {
            error!(%e, %retain_height, "Failed to prune the block store");
        }
    }
}

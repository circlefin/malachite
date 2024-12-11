//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use tokio::sync::mpsc;

use malachite_actors::util::events::TxEvent;
use malachite_node as node;

use crate::channel::AppMsg;
use crate::spawn::{
    spawn_block_sync_actor, spawn_consensus_actor, spawn_gossip_consensus_actor, spawn_host_actor,
    spawn_wal_actor,
};
use crate::types::codec::{BlockSyncCodec, ConsensusCodec, WalCodec};
use crate::types::config::Config as NodeConfig;
use crate::types::core::Context;
use crate::types::metrics::{Metrics, SharedRegistry};
use crate::types::Keypair;

#[allow(clippy::too_many_arguments)]
#[tracing::instrument("node", skip_all, fields(moniker = %cfg.moniker))]
pub async fn run<Node, Ctx, Codec>(
    cfg: NodeConfig,
    start_height: Option<Ctx::Height>,
    ctx: Ctx,
    codec: Codec,
    node: Node,
    keypair: Keypair,      // Todo: see note in code
    address: Ctx::Address, // Todo: remove it when Node was properly implemented
    initial_validator_set: Ctx::ValidatorSet,
) -> Result<mpsc::Receiver<AppMsg<Ctx>>, String>
where
    Ctx: Context,
    Node: node::Node<Context = Ctx>,
    Codec: WalCodec<Ctx> + Clone,
    Codec: ConsensusCodec<Ctx>,
    Codec: BlockSyncCodec<Ctx>,
{
    let start_height = start_height.unwrap_or_default();

    let registry = SharedRegistry::global().with_moniker(cfg.moniker.as_str());
    let metrics = Metrics::register(&registry);

    // The key types are not generic enough to create a gossip_consensus::KeyPair, but the current
    // libp2p implementation requires a KeyPair in SwarmBuilder::with_existing_identity.
    // We either decide on a specific keytype (ed25519 or ecdsa) or keep asking the user for the
    // KeyPair.
    // let private_key = node.load_private_key(node.load_private_key_file(&home_dir).unwrap());
    // let public_key = node.generate_public_key(private_key);
    // let address: Ctx::Address = node.get_address(public_key);
    // let pk_bytes = private_key.inner().to_bytes_be();
    // let secret_key = ecdsa::SecretKey::try_from_bytes(pk_bytes).unwrap();
    // let ecdsa_keypair = ecdsa::Keypair::from(secret_key);
    // Keypair::from(ecdsa_keypair)

    // Spawn consensus gossip
    let gossip_consensus =
        spawn_gossip_consensus_actor(&cfg, keypair, &registry, codec.clone()).await;

    let wal = spawn_wal_actor(&ctx, codec, &node.get_home_dir(), &registry).await;

    // Spawn the host actor
    let (connector, rx) = spawn_host_actor(metrics.clone()).await;

    let block_sync = spawn_block_sync_actor(
        ctx.clone(),
        gossip_consensus.clone(),
        connector.clone(),
        &cfg.blocksync,
        start_height,
        &registry,
    )
    .await;

    // Spawn consensus
    let _consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx,
        cfg,
        gossip_consensus,
        connector,
        wal,
        block_sync.clone(),
        metrics,
        TxEvent::new(),
    )
    .await;

    Ok(rx)
}

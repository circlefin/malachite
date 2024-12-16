//! The Application (or Node) definition. The Node trait implements the Consensus context and the
//! cryptographic library used for signing.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use eyre::eyre;
use libp2p_identity::Keypair;
use rand::{CryptoRng, RngCore};
use tracing::{debug, error};

use malachite_app_channel::app::consensus::ProposedValue;
use malachite_app_channel::app::types::core::{Round, Validity, VotingPower};
use malachite_app_channel::app::types::LocallyProposedValue;
use malachite_app_channel::app::Node;
use malachite_app_channel::{run, AppMsg, ConsensusGossipMsg, ConsensusMsg};
use malachite_test::{
    Address, Genesis, Height, PrivateKey, PublicKey, TestCodec, TestContext, Validator,
    ValidatorSet,
};
use malachite_test_cli::config::Config;

use crate::state::{decode_value, State};

#[derive(Clone)]
pub struct App {
    pub config: Config,
    pub home_dir: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<u64>,
}

#[async_trait]
impl Node for App {
    type Context = TestContext;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }

    fn get_address(&self, pk: &PublicKey) -> Address {
        Address::from_public_key(pk)
    }

    fn get_public_key(&self, pk: &PrivateKey) -> PublicKey {
        pk.public_key()
    }

    fn get_keypair(&self, pk: PrivateKey) -> Keypair {
        Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap()
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file
    }

    fn load_private_key_file(
        &self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(path)?;
        serde_json::from_str(&private_key).map_err(|e| e.into())
    }

    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        private_key
    }

    fn load_genesis(&self, path: impl AsRef<Path>) -> std::io::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(path)?;
        serde_json::from_str(&genesis).map_err(|e| e.into())
    }

    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }

    async fn run(&self) -> eyre::Result<()> {
        let span = tracing::error_span!("node", moniker = %self.config.moniker);
        let _enter = span.enter();

        let priv_key_file = self.load_private_key_file(self.private_key_file.clone())?;
        let private_key = self.load_private_key(priv_key_file);
        let address = self.get_address(&self.get_public_key(&private_key));
        let ctx = TestContext::new(private_key);

        let genesis = self.load_genesis(self.genesis_file.clone())?;

        let start_height = self.start_height.map(Height::new);

        let codec = TestCodec;

        let mut channels = run(
            self.config.clone(),
            start_height,
            ctx,
            codec,
            self.clone(),
            genesis.validator_set.clone(),
        )
        .await?;

        let mut state = State::new(address, start_height.unwrap_or_default());

        while let Some(msg) = channels.consensus.recv().await {
            match msg {
                AppMsg::ConsensusReady { reply_to } => {
                    debug!("Consensus is ready to run");
                    if reply_to
                        .send(ConsensusMsg::StartHeight(
                            state.current_height,
                            genesis.validator_set.clone(),
                        ))
                        .is_err()
                    {
                        error!("Failed to send ConsensusReady reply");
                    }
                }

                AppMsg::StartedRound {
                    height,
                    round,
                    proposer,
                } => {
                    state.current_height = height;
                    state.current_round = round;
                    state.current_proposer = Some(proposer);
                }

                AppMsg::GetValue {
                    height,
                    round: _,
                    timeout_duration: _,
                    address: _,
                    reply_to,
                } => {
                    let proposal = state.propose_value(&height);

                    let value = LocallyProposedValue::new(
                        proposal.height,
                        proposal.round,
                        proposal.value,
                        proposal.extension,
                    );

                    // Send it to consensus
                    if reply_to.send(value.clone()).is_err() {
                        error!("Failed to send GetValue reply");
                    }

                    let stream_message = state.create_broadcast_message(value);

                    // Broadcast it to others. Old messages need not be broadcast.
                    channels
                        .consensus_gossip
                        .send(ConsensusGossipMsg::PublishProposalPart(stream_message))
                        .await?;
                }

                AppMsg::GetEarliestBlockHeight { reply_to } => {
                    if reply_to.send(state.get_earliest_height()).is_err() {
                        error!("Failed to send GetEarliestBlockHeight reply");
                    }
                }

                AppMsg::ReceivedProposalPart {
                    from: _,
                    part,
                    reply_to,
                } => {
                    if let Some(proposed_value) = state.add_proposal(part) {
                        if reply_to.send(proposed_value).is_err() {
                            error!("Failed to send ReceivedProposalPart reply");
                        }
                    }
                }

                AppMsg::GetValidatorSet {
                    height: _,
                    reply_to,
                } => {
                    if reply_to.send(genesis.validator_set.clone()).is_err() {
                        error!("Failed to send GetValidatorSet reply");
                    }
                }

                AppMsg::Decided {
                    certificate,
                    reply_to,
                } => {
                    state.commit_block(certificate);
                    if reply_to
                        .send(ConsensusMsg::StartHeight(
                            state.current_height,
                            genesis.validator_set.clone(),
                        ))
                        .is_err()
                    {
                        error!("Failed to send Decided reply");
                    }
                }

                AppMsg::GetDecidedBlock { height, reply_to } => {
                    let block = state.get_block(&height).cloned();
                    if reply_to.send(block).is_err() {
                        error!("Failed to send GetDecidedBlock reply");
                    }
                }

                AppMsg::ProcessSyncedValue {
                    height,
                    round,
                    validator_address,
                    value_bytes,
                    reply_to,
                } => {
                    let value = decode_value(value_bytes);

                    if reply_to
                        .send(ProposedValue {
                            height,
                            round,
                            valid_round: Round::Nil,
                            validator_address,
                            value,
                            validity: Validity::Valid,
                            extension: None,
                        })
                        .is_err()
                    {
                        error!("Failed to send ProcessSyncedBlock reply");
                    }
                }

                AppMsg::RestreamValue { .. } => {
                    unimplemented!("RestreamValue");
                }
            }
        }

        Err(eyre!("Consensus channel closed unexpectedly"))
    }
}

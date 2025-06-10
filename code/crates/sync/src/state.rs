use std::collections::BTreeMap;

use libp2p::StreamProtocol;
use rand::seq::IteratorRandom;

use malachitebft_core_types::{Context, Height};
use malachitebft_peer::PeerId;
use tracing::warn;

use crate::{Behaviour, PeerDetails, PeerKind, Status};

pub struct State<Ctx>
where
    Ctx: Context,
{
    rng: Box<dyn rand::RngCore + Send>,

    /// Consensus has started
    pub started: bool,

    /// Height of last decided value
    pub tip_height: Ctx::Height,

    /// Height currently syncing.
    pub sync_height: Ctx::Height,

    /// Decided value requests for these heights have been sent out to peers.
    pub pending_decided_value_requests: BTreeMap<Ctx::Height, PeerId>,

    /// The set of peers we are connected to in order to get values, certificates and votes.
    /// TODO - For now value and vote sync peers are the same. Might need to revise in the future.
    pub peers: BTreeMap<PeerId, PeerDetails<Ctx>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(rng: Box<dyn rand::RngCore + Send>) -> Self {
        Self {
            rng,
            started: false,
            tip_height: Ctx::Height::ZERO,
            sync_height: Ctx::Height::ZERO,
            pending_decided_value_requests: BTreeMap::new(),
            peers: BTreeMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer_id: PeerId, protocols: Vec<StreamProtocol>) {
        if self.peers.contains_key(&peer_id) {
            warn!("Peer {} already exists in the state", peer_id);
            return;
        }

        let kind = if protocols.contains(&Behaviour::SYNC_V2_PROTOCOL.0) {
            PeerKind::SyncV2
        } else if protocols.contains(&Behaviour::SYNC_V1_PROTOCOL.0) {
            PeerKind::SyncV1
        } else {
            warn!("Peer {} does not support any known sync protocol", peer_id);
            return;
        };

        self.peers.insert(
            peer_id,
            PeerDetails {
                kind,
                status: Status::default(peer_id),
            },
        );
    }

    pub fn update_status(&mut self, status: Status<Ctx>) {
        // TODO: should we consider status messages from non connected peers?
        if let Some(peer_details) = self.peers.get_mut(&status.peer_id) {
            peer_details.update_status(status);
        }
    }

    /// Select at random a peer whose tip is at or above the given height and with min height below the given height.
    /// In other words, `height` is in `status.history_min_height..=status.tip_height` range.
    pub fn random_peer_with_tip_at_or_above(&mut self, height: Ctx::Height) -> Option<PeerId>
    where
        Ctx: Context,
    {
        self.peers
            .iter()
            .filter_map(|(&peer, detail)| {
                (detail.status.history_min_height..=detail.status.tip_height)
                    .contains(&height)
                    .then_some(peer)
            })
            .choose_stable(&mut self.rng)
    }

    /// Same as [`Self::random_peer_with_tip_at_or_above`], but excludes the given peer.
    pub fn random_peer_with_tip_at_or_above_except(
        &mut self,
        height: Ctx::Height,
        except: PeerId,
    ) -> Option<PeerId> {
        self.peers
            .iter()
            .filter_map(|(&peer, detail)| {
                (detail.status.history_min_height..=detail.status.tip_height)
                    .contains(&height)
                    .then_some(peer)
            })
            .filter(|&peer| peer != except)
            .choose_stable(&mut self.rng)
    }

    pub fn store_pending_decided_value_request(&mut self, height: Ctx::Height, peer: PeerId) {
        self.pending_decided_value_requests.insert(height, peer);
    }

    pub fn store_pending_decided_batch_request(
        &mut self,
        from: Ctx::Height,
        to: Ctx::Height,
        peer: PeerId,
    ) {
        let mut height = from;
        loop {
            self.pending_decided_value_requests.insert(height, peer);
            if height >= to {
                break;
            }
            height = height.increment();
        }
    }

    pub fn remove_pending_decided_value_request(&mut self, height: Ctx::Height) {
        self.pending_decided_value_requests.remove(&height);
    }

    pub fn remove_pending_decided_batch_request(&mut self, from: Ctx::Height, to: Ctx::Height) {
        let mut height = from;
        while height <= to {
            self.pending_decided_value_requests.remove(&height);
            height = height.increment();
        }
    }

    pub fn has_pending_decided_value_request(&self, height: &Ctx::Height) -> bool {
        self.pending_decided_value_requests.contains_key(height)
    }
}

#![allow(clippy::bool_assert_comparison)]

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::time::Duration;

use libp2p::{request_response::OutboundRequestId, swarm::ConnectionId, Multiaddr, PeerId};
use tokio::sync::mpsc;
use tracing::error;

use crate::{request::RequestData, DialData};

const DEFAULT_DIAL_CONCURRENT_FACTOR: usize = 20;
const DEFAULT_PEERS_REQUEST_CONCURRENT_FACTOR: usize = 20;
const DEFAULT_CONNECT_REQUEST_CONCURRENT_FACTOR: usize = 100;
const DEFAULT_CLOSE_CONCURRENT_FACTOR: usize = usize::MAX;

#[derive(Debug)]
pub struct Action<T, U, V> {
    tx_queue: mpsc::UnboundedSender<V>,
    rx_queue: mpsc::UnboundedReceiver<V>,
    done_on: HashSet<T>,
    concurrent_factor: usize,
    in_progress: HashMap<U, V>,
}

impl<T, U, V> Action<T, U, V>
where
    T: Eq + Hash,
    U: Eq + Hash,
    V: Send + 'static,
{
    pub(crate) fn new(concurrent_factor: usize) -> Self {
        let (tx_queue, rx_queue) = mpsc::unbounded_channel();

        Self {
            tx_queue,
            rx_queue,
            done_on: HashSet::new(),
            concurrent_factor,
            in_progress: HashMap::new(),
        }
    }

    pub(crate) fn add_to_queue(&mut self, value: V, delay: Option<Duration>) {
        // Avoid spawning a new task if the delay is None
        if delay.is_none() {
            self.tx_queue.send(value).unwrap_or_else(|e| {
                error!("Failed to send value to queue: {:?}", e);
            });
            return;
        }

        let tx_queue = self.tx_queue.clone();
        tokio::spawn(async move {
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
            tx_queue.send(value).unwrap_or_else(|e| {
                error!("Failed to send value to queue: {:?}", e);
            })
        });
    }

    pub(crate) fn queue_len(&self) -> usize {
        self.rx_queue.len()
    }

    pub async fn recv(&mut self) -> Option<V> {
        self.rx_queue.recv().await
    }

    pub(crate) fn register_done_on(&mut self, key: T) {
        self.done_on.insert(key);
    }

    pub(crate) fn is_done_on(&self, key: &T) -> bool {
        self.done_on.contains(key)
    }

    pub(crate) fn can_perform(&self) -> bool {
        self.in_progress.len() < self.concurrent_factor
    }

    pub(crate) fn register_in_progress(&mut self, key: U, value: V) {
        self.in_progress.insert(key, value);
    }

    pub(crate) fn get_in_progress_iter(&self) -> impl Iterator<Item = (&U, &V)> {
        self.in_progress.iter()
    }

    pub(crate) fn is_in_progress(&self, key: &U) -> bool {
        self.in_progress.contains_key(key)
    }

    pub(crate) fn get_in_progress_mut(&mut self, key: &U) -> Option<&mut V> {
        self.in_progress.get_mut(key)
    }

    pub(crate) fn remove_in_progress(&mut self, key: &U) -> Option<V> {
        self.in_progress.remove(key)
    }

    pub(crate) fn is_idle(&self) -> (bool, usize) {
        (self.in_progress.is_empty(), self.in_progress.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PeerData {
    PeerId(PeerId),
    Multiaddr(Multiaddr),
}

#[derive(Debug)]
pub struct Controller {
    pub dial: Action<PeerData, ConnectionId, DialData>,
    pub peers_request: Action<PeerId, OutboundRequestId, RequestData>,
    pub connect_request: Action<PeerId, OutboundRequestId, RequestData>,
    pub close: Action<(), (), (PeerId, ConnectionId)>,
}

impl Controller {
    pub(crate) fn new() -> Self {
        Controller {
            dial: Action::new(DEFAULT_DIAL_CONCURRENT_FACTOR),
            peers_request: Action::new(DEFAULT_PEERS_REQUEST_CONCURRENT_FACTOR),
            connect_request: Action::new(DEFAULT_CONNECT_REQUEST_CONCURRENT_FACTOR),
            close: Action::new(DEFAULT_CLOSE_CONCURRENT_FACTOR),
        }
    }

    pub(crate) fn dial_register_done_on(&mut self, dial_data: &DialData) {
        if let Some(peer_id) = dial_data.peer_id() {
            self.dial.register_done_on(PeerData::PeerId(peer_id));
        }
        for addr in dial_data.listen_addrs() {
            self.dial
                .register_done_on(PeerData::Multiaddr(addr.clone()));
        }
    }

    pub(crate) fn dial_is_done_on(&self, dial_data: &DialData) -> bool {
        dial_data
            .peer_id()
            .is_some_and(|peer_id| self.dial.is_done_on(&PeerData::PeerId(peer_id)))
            || dial_data
                .listen_addrs()
                .iter()
                .any(|addr| self.dial.is_done_on(&PeerData::Multiaddr(addr.clone())))
    }

    pub(crate) fn dial_add_peer_id_to_dial_data(
        &mut self,
        connection_id: ConnectionId,
        peer_id: PeerId,
    ) {
        if let Some(dial_data) = self.dial.get_in_progress_mut(&connection_id) {
            dial_data.set_peer_id(peer_id);
        }
    }

    pub(crate) fn dial_remove_matching_in_progress_connections(
        &mut self,
        peer_id: &PeerId,
    ) -> Vec<DialData> {
        let matching_connection_ids = self
            .dial
            .get_in_progress_iter()
            .filter_map(|(connection_id, dial_data)| {
                if dial_data.peer_id() == Some(*peer_id) {
                    Some(*connection_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        matching_connection_ids
            .into_iter()
            .filter_map(|connection_id| self.dial.remove_in_progress(&connection_id))
            .collect()
    }

    pub(crate) fn is_idle(&self) -> (bool, usize, usize) {
        let (is_dial_idle, in_progress_dial_len) = self.dial.is_idle();
        let (is_peers_request_idle, in_progress_peers_request_len) = self.peers_request.is_idle();
        (
            is_dial_idle && is_peers_request_idle,
            in_progress_dial_len,
            in_progress_peers_request_len,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_action() {
        let mut action = Action::<PeerData, u32, u32>::new(2);

        assert_eq!(action.can_perform(), true);
        assert_eq!(action.is_idle(), (true, 0));

        let peer_id = PeerId::random();
        let multiaddr = Multiaddr::from_str("/ip4/127.0.0.1/tcp/12345").unwrap();

        assert_eq!(action.is_done_on(&PeerData::PeerId(peer_id)), false);
        assert_eq!(
            action.is_done_on(&PeerData::Multiaddr(multiaddr.clone())),
            false
        );

        action.register_in_progress(1, 1);

        assert_eq!(action.can_perform(), true);
        assert_eq!(action.is_idle(), (false, 1));

        action.register_in_progress(2, 2);

        assert_eq!(action.can_perform(), false);
        assert_eq!(action.is_idle(), (false, 2));

        assert_eq!(action.remove_in_progress(&1), Some(1));
        assert_eq!(action.can_perform(), true);
        assert_eq!(action.is_idle(), (false, 1));
        assert_eq!(action.remove_in_progress(&1), None);

        action.register_done_on(PeerData::PeerId(peer_id));

        assert_eq!(action.is_done_on(&PeerData::PeerId(peer_id)), true);
        assert_eq!(
            action.is_done_on(&PeerData::Multiaddr(multiaddr.clone())),
            false
        );

        action.register_done_on(PeerData::Multiaddr(multiaddr.clone()));

        assert_eq!(action.is_done_on(&PeerData::PeerId(peer_id)), true);
        assert_eq!(
            action.is_done_on(&PeerData::Multiaddr(multiaddr.clone())),
            true
        );

        assert_eq!(action.remove_in_progress(&2), Some(2));
        assert_eq!(action.can_perform(), true);
        assert_eq!(action.is_idle(), (true, 0));
        assert_eq!(action.remove_in_progress(&2), None);
    }
}

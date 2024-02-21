use core::fmt;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures::channel::oneshot;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

use super::{Msg, Network, PeerId};

pub enum PeerEvent {
    ConnectToPeer(PeerInfo, Option<Duration>, oneshot::Sender<()>),
    Broadcast(Msg, oneshot::Sender<()>),
}

impl Debug for PeerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeerEvent::ConnectToPeer(peer_info, _, _) => {
                write!(f, "ConnectToPeer({peer_info:?})")
            }
            PeerEvent::Broadcast(msg, _) => {
                write!(f, "Broadcast({msg:?})")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerInfo {
    pub id: PeerId,
    pub addr: SocketAddr,
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{id} ({addr})", id = self.id, addr = self.addr)
    }
}

pub struct Peer {
    id: PeerId,
    addr: SocketAddr,
}

impl Peer {
    pub fn new(info: PeerInfo) -> Self {
        Self {
            id: info.id,
            addr: info.addr,
        }
    }

    pub async fn run(self) -> Handle {
        let (tx_peer_event, mut rx_peer_event) = mpsc::channel::<PeerEvent>(16);
        let (tx_msg, rx_msg) = mpsc::channel::<(PeerId, Msg)>(16);
        let (tx_broadcast_to_peers, _) = broadcast::channel::<(PeerId, Msg)>(16);
        let (tx_spawned, rx_spawned) = oneshot::channel();

        tokio::spawn(listen(self.id.clone(), self.addr, tx_spawned, tx_msg));

        let id = self.id.clone();

        tokio::spawn(async move {
            while let Some(event) = rx_peer_event.recv().await {
                match event {
                    PeerEvent::ConnectToPeer(peer_info, timeout, done) => {
                        connect_to_peer(
                            id.clone(),
                            peer_info,
                            timeout,
                            done,
                            &tx_broadcast_to_peers,
                        )
                        .await;
                    }

                    PeerEvent::Broadcast(msg, done) => {
                        debug!("[{id}] Broadcasting message: {msg:?}");
                        tx_broadcast_to_peers.send((id.clone(), msg)).unwrap();
                        done.send(()).unwrap();
                    }
                }
            }
        });

        rx_spawned.await.unwrap();

        Handle {
            peer_id: self.id,
            rx_msg,
            tx_peer_event,
        }
    }
}

async fn connect_to_peer(
    id: PeerId,
    peer_info: PeerInfo,
    timeout: Option<Duration>,
    done: oneshot::Sender<()>,
    per_peer_tx: &broadcast::Sender<(PeerId, Msg)>,
) {
    info!("[{id}] Connecting to {peer_info}...");

    let mut stream = if let Some(timeout) = timeout {
        let start = Instant::now();

        loop {
            match TcpStream::connect(peer_info.addr).await {
                Ok(stream) => break stream,
                Err(e) => warn!("[{id}] Failed to connect to {peer_info}: {e}"),
            }

            if start.elapsed() >= timeout {
                error!("[{id}] Connection to {peer_info} timed out");
                return;
            }

            warn!("[{id}] Retrying in 1 second...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    } else {
        match TcpStream::connect(peer_info.addr).await {
            Ok(stream) => stream,
            Err(e) => {
                error!("[{id}] Failed to connect to {peer_info}: {e}");
                return;
            }
        }
    };

    done.send(()).unwrap();

    let mut per_peer_rx = per_peer_tx.subscribe();

    Frame::PeerId(id.clone()).write(&mut stream).await.unwrap();

    tokio::spawn(async move {
        loop {
            let (from, msg) = per_peer_rx.recv().await.unwrap();
            if from == peer_info.id {
                continue;
            }

            debug!("[{id}] Sending message to {peer_info}: {msg:?}");
            Frame::Msg(msg).write(&mut stream).await.unwrap();
        }
    });
}

async fn listen(
    id: PeerId,
    addr: SocketAddr,
    tx_spawned: oneshot::Sender<()>,
    tx_received: mpsc::Sender<(PeerId, Msg)>,
) -> ! {
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("[{id}] Listening on {addr}...");

    tx_spawned.send(()).unwrap();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();

        info!(
            "[{id}] Accepted connection from {peer}...",
            peer = socket.peer_addr().unwrap()
        );

        let Frame::PeerId(peer_id) = Frame::read(&mut socket).await.unwrap() else {
            error!("[{id}] Peer did not send its ID");
            continue;
        };

        let id = id.clone();
        let tx_received = tx_received.clone();

        tokio::spawn(async move {
            loop {
                let Frame::Msg(msg) = Frame::read(&mut socket).await.unwrap() else {
                    error!("[{id}] Peer did not send a message");
                    return;
                };

                debug!(
                    "[{id}] Received message from {peer_id} ({addr}): {msg:?}",
                    addr = socket.peer_addr().unwrap(),
                );

                tx_received.send((peer_id.clone(), msg)).await.unwrap(); // FIXME
            }
        });
    }
}

pub enum Frame {
    PeerId(PeerId),
    Msg(Msg),
}

impl Frame {
    /// Write a frame to the given writer, prefixing it with its discriminant.
    pub async fn write<W: AsyncWriteExt + Unpin>(
        &self,
        writer: &mut W,
    ) -> Result<(), std::io::Error> {
        match self {
            Frame::PeerId(id) => {
                writer.write_u8(0x40).await?;
                let bytes = id.0.as_bytes();
                writer.write_u32(bytes.len() as u32).await?;
                writer.write_all(bytes).await?;
                writer.flush().await?;
            }
            Frame::Msg(msg) => {
                writer.write_u8(0x41).await?;
                let bytes = msg
                    .to_network_bytes()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                writer.write_u32(bytes.len() as u32).await?;
                writer.write_all(&bytes).await?;
                writer.flush().await?;
            }
        }

        Ok(())
    }

    pub async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self, std::io::Error> {
        let discriminant = reader.read_u8().await?;

        match discriminant {
            0x40 => {
                let len = reader.read_u32().await?;
                let mut buf = vec![0; len as usize];
                reader.read_exact(&mut buf).await?;
                Ok(Frame::PeerId(PeerId(String::from_utf8(buf).unwrap())))
            }
            0x41 => {
                let len = reader.read_u32().await?;
                let mut buf = vec![0; len as usize];
                reader.read_exact(&mut buf).await?;
                Ok(Frame::Msg(Msg::from_network_bytes(&buf).unwrap()))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid frame discriminant: {discriminant}"),
            )),
        }
    }
}

pub struct Handle {
    peer_id: PeerId,
    rx_msg: mpsc::Receiver<(PeerId, Msg)>,
    tx_peer_event: mpsc::Sender<PeerEvent>,
}

impl Handle {
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    pub async fn recv(&mut self) -> Option<(PeerId, Msg)> {
        self.rx_msg.recv().await
    }

    pub async fn broadcast(&self, msg: Msg) {
        let (tx_done, rx_done) = oneshot::channel();

        self.tx_peer_event
            .send(PeerEvent::Broadcast(msg, tx_done))
            .await
            .unwrap();

        rx_done.await.unwrap();
    }

    pub async fn connect_to_peer(&self, peer_info: PeerInfo, timeout: Option<Duration>) {
        let (tx_done, rx_done) = oneshot::channel();

        self.tx_peer_event
            .send(PeerEvent::ConnectToPeer(peer_info, timeout, tx_done))
            .await
            .unwrap();

        rx_done.await.unwrap();
    }
}

impl Network for Handle {
    async fn recv(&mut self) -> Option<(PeerId, Msg)> {
        Handle::recv(self).await
    }

    async fn broadcast(&mut self, msg: Msg) {
        Handle::broadcast(self, msg).await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    #[tokio::test]
    async fn test_peer() {
        let peer1_id = PeerId("peer-1".to_string());
        let peer1_info = PeerInfo {
            id: peer1_id.clone(),
            addr: "127.0.0.1:12001".parse().unwrap(),
        };

        let peer2_id = PeerId("peer-2".to_string());
        let peer2_info = PeerInfo {
            id: peer2_id.clone(),
            addr: "127.0.0.1:12002".parse().unwrap(),
        };

        let peer3_id = PeerId("peer-3".to_string());
        let peer3_info = PeerInfo {
            id: peer3_id.clone(),
            addr: "127.0.0.1:12003".parse().unwrap(),
        };

        let peer1: Peer = Peer::new(peer1_info.clone());
        let peer2: Peer = Peer::new(peer2_info.clone());
        let peer3: Peer = Peer::new(peer3_info.clone());

        let handle1 = peer1.run().await;
        let mut handle2 = peer2.run().await;
        let mut handle3 = peer3.run().await;

        handle1.connect_to_peer(peer2_info.clone(), None).await;
        handle1.connect_to_peer(peer3_info.clone(), None).await;

        handle2.connect_to_peer(peer1_info.clone(), None).await;
        handle2.connect_to_peer(peer3_info.clone(), None).await;

        handle3.connect_to_peer(peer1_info.clone(), None).await;
        handle3.connect_to_peer(peer2_info.clone(), None).await;

        handle1.broadcast(Msg::Dummy(1)).await;
        handle1.broadcast(Msg::Dummy(2)).await;

        let deadline = Duration::from_millis(100);

        let msg2 = timeout(deadline, handle2.recv()).await.unwrap();
        dbg!(&msg2);
        let msg3 = timeout(deadline, handle3.recv()).await.unwrap();
        dbg!(&msg3);

        let msg4 = timeout(deadline, handle2.recv()).await.unwrap();
        dbg!(&msg4);
        let msg5 = timeout(deadline, handle3.recv()).await.unwrap();
        dbg!(&msg5);
    }
}

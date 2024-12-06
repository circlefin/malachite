use bytes::Bytes;
use libp2p::request_response::InboundRequestId;
use tokio::sync::{mpsc, oneshot};
use tokio::task;

use malachite_blocksync::OutboundRequestId;
use malachite_peer::PeerId;

use crate::{Channel, CtrlMsg, Event};

pub struct RecvHandle {
    peer_id: PeerId,
    rx_event: mpsc::Receiver<Event>,
}

impl RecvHandle {
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub async fn recv(&mut self) -> Option<Event> {
        self.rx_event.recv().await
    }
}

pub struct CtrlHandle {
    peer_id: PeerId,
    tx_ctrl: mpsc::Sender<CtrlMsg>,
    task_handle: task::JoinHandle<()>,
}

impl CtrlHandle {
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub async fn publish(&self, channel: Channel, data: Bytes) -> Result<(), eyre::Report> {
        self.tx_ctrl.send(CtrlMsg::Publish(channel, data)).await?;
        Ok(())
    }

    pub async fn broadcast(&self, channel: Channel, data: Bytes) -> Result<(), eyre::Report> {
        self.tx_ctrl.send(CtrlMsg::Broadcast(channel, data)).await?;
        Ok(())
    }

    pub async fn blocksync_request(
        &self,
        peer_id: PeerId,
        data: Bytes,
    ) -> Result<OutboundRequestId, eyre::Report> {
        let (tx, rx) = oneshot::channel();

        self.tx_ctrl
            .send(CtrlMsg::BlockSyncRequest(peer_id, data, tx))
            .await?;

        Ok(rx.await?)
    }

    pub async fn blocksync_reply(
        &self,
        request_id: InboundRequestId,
        data: Bytes,
    ) -> Result<(), eyre::Report> {
        self.tx_ctrl
            .send(CtrlMsg::BlockSyncReply(request_id, data))
            .await?;
        Ok(())
    }

    pub async fn wait_shutdown(self) -> Result<(), eyre::Report> {
        self.shutdown().await?;
        self.join().await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), eyre::Report> {
        self.tx_ctrl.send(CtrlMsg::Shutdown).await?;
        Ok(())
    }

    pub async fn join(self) -> Result<(), eyre::Report> {
        self.task_handle.await?;
        Ok(())
    }
}

pub struct Handle {
    peer_id: PeerId,
    recv: RecvHandle,
    ctrl: CtrlHandle,
}

impl Handle {
    pub fn new(
        peer_id: PeerId,
        tx_ctrl: mpsc::Sender<CtrlMsg>,
        rx_event: mpsc::Receiver<Event>,
        task_handle: task::JoinHandle<()>,
    ) -> Self {
        Self {
            peer_id,
            recv: RecvHandle { peer_id, rx_event },
            ctrl: CtrlHandle {
                peer_id,
                tx_ctrl,
                task_handle,
            },
        }
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn split(self) -> (RecvHandle, CtrlHandle) {
        (self.recv, self.ctrl)
    }

    pub async fn recv(&mut self) -> Option<Event> {
        self.recv.recv().await
    }

    pub async fn broadcast(&self, channel: Channel, data: Bytes) -> Result<(), eyre::Report> {
        self.ctrl.publish(channel, data).await
    }

    pub async fn wait_shutdown(self) -> Result<(), eyre::Report> {
        self.ctrl.wait_shutdown().await
    }

    pub async fn shutdown(&self) -> Result<(), eyre::Report> {
        self.ctrl.shutdown().await
    }

    pub async fn join(self) -> Result<(), eyre::Report> {
        self.ctrl.join().await
    }
}

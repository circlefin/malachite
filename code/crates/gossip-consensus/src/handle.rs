use malachite_common::Context;
use malachite_consensus::GossipMsg;
use tokio::sync::mpsc;
use tokio::task;

use crate::{BoxError, Channel, CtrlMsg, Event};

pub struct RecvHandle<Ctx: Context> {
    rx_event: mpsc::Receiver<Event<Ctx>>,
}

impl<Ctx: Context> RecvHandle<Ctx> {
    pub async fn recv(&mut self) -> Option<Event<Ctx>> {
        self.rx_event.recv().await
    }
}

pub struct CtrlHandle<Ctx: Context> {
    tx_ctrl: mpsc::Sender<CtrlMsg<Ctx>>,
    task_handle: task::JoinHandle<()>,
}

impl<Ctx: Context> CtrlHandle<Ctx> {
    pub async fn broadcast(&self, channel: Channel, msg: GossipMsg<Ctx>) -> Result<(), BoxError> {
        self.tx_ctrl.send(CtrlMsg::Broadcast(channel, msg)).await?;
        Ok(())
    }

    pub async fn wait_shutdown(self) -> Result<(), BoxError> {
        self.shutdown().await?;
        self.join().await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), BoxError> {
        self.tx_ctrl.send(CtrlMsg::<Ctx>::Shutdown).await?;
        Ok(())
    }

    pub async fn join(self) -> Result<(), BoxError> {
        self.task_handle.await?;
        Ok(())
    }
}

pub struct Handle<Ctx: Context> {
    recv: RecvHandle<Ctx>,
    ctrl: CtrlHandle<Ctx>,
}

impl<Ctx: Context> Handle<Ctx> {
    pub fn new(
        tx_ctrl: mpsc::Sender<CtrlMsg<Ctx>>,
        rx_event: mpsc::Receiver<Event<Ctx>>,
        task_handle: task::JoinHandle<()>,
    ) -> Self {
        Self {
            recv: RecvHandle { rx_event },
            ctrl: CtrlHandle {
                tx_ctrl,
                task_handle,
            },
        }
    }

    pub fn split(self) -> (RecvHandle<Ctx>, CtrlHandle<Ctx>) {
        (self.recv, self.ctrl)
    }

    pub async fn recv(&mut self) -> Option<Event<Ctx>> {
        self.recv.recv().await
    }

    pub async fn broadcast(&self, channel: Channel, msg: GossipMsg<Ctx>) -> Result<(), BoxError> {
        self.ctrl.broadcast(channel, msg).await
    }

    pub async fn wait_shutdown(self) -> Result<(), BoxError> {
        self.ctrl.wait_shutdown().await
    }

    pub async fn shutdown(&self) -> Result<(), BoxError> {
        self.ctrl.shutdown().await
    }

    pub async fn join(self) -> Result<(), BoxError> {
        self.ctrl.join().await
    }
}

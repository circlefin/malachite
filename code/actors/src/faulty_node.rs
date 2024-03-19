#![allow(dead_code, unused_variables)]

use std::fmt::Display;
use std::time::Duration;

use ractor::Actor;
use ractor::ActorProcessingErr;
use ractor::ActorRef;
use rand::seq::IteratorRandom;
use rand::Rng;
use tracing::warn;

use malachite_common::Context;
use malachite_proto::{self as proto, Protobuf};

use crate::node::Msg;
use crate::node::Node;
use crate::node::State;

pub type Prob = f64;

#[derive(Clone, Debug)]
pub struct Faults {
    faults: Vec<Fault>,
}

impl Faults {
    pub fn new(faults: Vec<Fault>) -> Self {
        Self { faults }
    }

    pub fn choose_fault_for_msg<Ctx>(
        &self,
        msg: &Msg<Ctx>,
        rng: &mut dyn rand::RngCore,
    ) -> Option<&Fault>
    where
        Ctx: Context,
    {
        self.faults
            .iter()
            .filter(|f| f.applies_to_msg(msg))
            .choose_stable(rng)
            .filter(|fault| fault.is_enabled(rng))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Fault {
    DiscardGossipEvent(Prob),

    DelayStartHeight(Prob, Duration),
    DelayMoveToNextHeight(Prob, Duration),

    DiscardDriverInput(Prob),
    DelayDriverInput(Prob, Duration),

    DiscardDriverOutputs(Prob),
    DelayDriverOutputs(Prob, Duration),

    DiscardProposeValue(Prob),
    DelayProposeValue(Prob, Duration),
}

impl Fault {
    pub fn is_enabled(&self, rng: &mut dyn rand::RngCore) -> bool {
        match self {
            Fault::DiscardGossipEvent(prob)
            | Fault::DelayStartHeight(prob, _)
            | Fault::DelayMoveToNextHeight(prob, _)
            | Fault::DiscardDriverInput(prob)
            | Fault::DelayDriverInput(prob, _)
            | Fault::DiscardDriverOutputs(prob)
            | Fault::DelayDriverOutputs(prob, _)
            | Fault::DiscardProposeValue(prob)
            | Fault::DelayProposeValue(prob, _) => rng.gen_bool(*prob),
        }
    }

    pub fn applies_to_msg<Ctx>(&self, msg: &Msg<Ctx>) -> bool
    where
        Ctx: Context,
    {
        match self {
            Fault::DiscardGossipEvent(_) => matches!(msg, Msg::GossipEvent(_)),
            Fault::DelayStartHeight(_, _) => matches!(msg, Msg::StartHeight(_)),
            Fault::DelayMoveToNextHeight(_, _) => matches!(msg, Msg::MoveToNextHeight),
            Fault::DiscardDriverInput(_) | Fault::DelayDriverInput(_, _) => {
                matches!(msg, Msg::SendDriverInput(_))
            }
            Fault::DiscardDriverOutputs(_) | Fault::DelayDriverOutputs(_, _) => {
                matches!(msg, Msg::ProcessDriverOutputs(_, _))
            }
            Fault::DiscardProposeValue(_) | Fault::DelayProposeValue(_, _) => {
                matches!(msg, Msg::ProposeValue(_, _, _))
            }
        }
    }
}

pub struct FaultyState<Ctx>
where
    Ctx: Context,
{
    node_state: State<Ctx>,
    rng: Box<dyn rand::RngCore + Send + Sync>,
}

pub struct FaultyArgs {
    rng: Box<dyn rand::RngCore + Send + Sync>,
}

pub struct FaultyNode<Ctx>
where
    Ctx: Context,
{
    node: Node<Ctx>,
    faults: Faults,
}

impl<Ctx> FaultyNode<Ctx>
where
    Ctx: Context,
    Ctx::Height: Display,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
{
    pub fn new(node: Node<Ctx>, faults: Vec<Fault>) -> Self {
        Self {
            node,
            faults: Faults::new(faults),
        }
    }

    pub async fn spawn(
        node: Node<Ctx>,
        faults: Vec<Fault>,
        rng: Box<dyn rand::RngCore + Send + Sync>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let faulty_node = Self::new(node, faults);
        let (actor_ref, _) = Actor::spawn(None, faulty_node, FaultyArgs { rng }).await?;

        Ok(actor_ref)
    }
}

#[ractor::async_trait]
impl<Ctx> Actor for FaultyNode<Ctx>
where
    Ctx: Context,
    Ctx::Height: Display,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
{
    type Msg = Msg<Ctx>;
    type State = FaultyState<Ctx>;
    type Arguments = FaultyArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: FaultyArgs,
    ) -> Result<Self::State, ActorProcessingErr> {
        let state = self.node.pre_start(myself, ()).await?;

        Ok(FaultyState {
            node_state: state,
            rng: args.rng,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut FaultyState<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        if let Some(fault) = self.faults.choose_fault_for_msg(&msg, &mut state.rng) {
            warn!("Injecting fault: {fault:?}");

            match (&msg, fault) {
                (Msg::GossipEvent(_), Fault::DiscardGossipEvent(_)) => {
                    // Do nothing
                    Ok(())
                }

                (Msg::StartHeight(_), Fault::DelayStartHeight(_, delay)) => {
                    tokio::time::sleep(*delay).await;
                    self.node.handle(myself, msg, &mut state.node_state).await
                }

                (Msg::MoveToNextHeight, Fault::DelayMoveToNextHeight(_, delay)) => {
                    tokio::time::sleep(*delay).await;
                    self.node.handle(myself, msg, &mut state.node_state).await
                }

                (Msg::SendDriverInput(_), Fault::DiscardDriverInput(_)) => Ok(()),
                (Msg::SendDriverInput(_), Fault::DelayDriverInput(_, delay)) => {
                    tokio::time::sleep(*delay).await;
                    self.node.handle(myself, msg, &mut state.node_state).await
                }

                (Msg::ProcessDriverOutputs(_, _), Fault::DiscardDriverOutputs(_)) => Ok(()),
                (Msg::ProcessDriverOutputs(_, _), Fault::DelayDriverOutputs(_, delay)) => {
                    tokio::time::sleep(*delay).await;
                    self.node.handle(myself, msg, &mut state.node_state).await
                }

                (Msg::ProposeValue(_, _, _), Fault::DiscardProposeValue(_)) => Ok(()),
                (Msg::ProposeValue(_, _, _), Fault::DelayProposeValue(_, delay)) => {
                    tokio::time::sleep(*delay).await;
                    self.node.handle(myself, msg, &mut state.node_state).await
                }

                // Wrong combination of message and fault, just handle the message normally.
                // This should never happen, but oh well.
                _ => self.node.handle(myself, msg, &mut state.node_state).await,
            }
        } else {
            self.node.handle(myself, msg, &mut state.node_state).await
        }
    }
}

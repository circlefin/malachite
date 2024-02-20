use derive_where::derive_where;

use malachite_common::Context;

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Msg<Ctx: Context> {
    Vote(Ctx::Vote),
    Proposal(Ctx::Proposal),

    #[cfg(test)]
    Dummy(u32),
}

impl<Ctx: Context> Msg<Ctx> {
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Msg::Vote(_vote) => todo!(),
            Msg::Proposal(_proposal) => todo!(),

            #[cfg(test)]
            Msg::Dummy(n) => [&[0x42], n.to_be_bytes().as_slice()].concat(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        match bytes {
            #[cfg(test)]
            [0x42, a, b, c, d] => Msg::Dummy(u32::from_be_bytes([*a, *b, *c, *d])),

            _ => todo!(),
        }
    }
}
//
// impl<Ctx: Context> Clone for Msg<Ctx> {
//     fn clone(&self) -> Self {
//         match self {
//             Msg::Vote(vote) => Msg::Vote(vote.clone()),
//             Msg::Proposal(proposal) => Msg::Proposal(proposal.clone()),
//
//             #[cfg(test)]
//             Msg::Dummy(n) => Msg::Dummy(*n),
//         }
//     }
// }
//
// impl<Ctx: Context> fmt::Debug for Msg<Ctx> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Msg::Vote(vote) => write!(f, "Vote({vote:?})"),
//             Msg::Proposal(proposal) => write!(f, "Proposal({proposal:?})"),
//
//             #[cfg(test)]
//             Msg::Dummy(n) => write!(f, "Dummy({n:?})"),
//         }
//     }
// }
//
// impl<Ctx: Context> PartialEq for Msg<Ctx> {
//     fn eq(&self, other: &Self) -> bool {
//         match (self, other) {
//             (Msg::Vote(vote), Msg::Vote(other_vote)) => vote == other_vote,
//             (Msg::Proposal(proposal), Msg::Proposal(other_proposal)) => proposal == other_proposal,
//
//             #[cfg(test)]
//             (Msg::Dummy(n1), Msg::Dummy(n2)) => n1 == n2,
//
//             _ => false,
//         }
//     }
// }
//
// impl<Ctx: Context> Eq for Msg<Ctx> {}

use std::time::SystemTime;

use derive_where::derive_where;
use displaydoc::Display;

use malachite_common::{Context, Round};

use super::Line;

#[derive_where(Clone, Debug, Eq, PartialEq)]
#[derive(Display)]
/// height: {height}, round: {round}, line: {line}
pub struct Trace<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub time: SystemTime,
    pub line: Line,
}

impl<Ctx: Context> Trace<Ctx> {
    pub fn new(height: Ctx::Height, round: Round, line: Line) -> Self {
        Self {
            height,
            round,
            time: SystemTime::now(),
            line,
        }
    }
}

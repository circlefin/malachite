use derive_where::derive_where;
use displaydoc::Display;
use time::OffsetDateTime;

use malachite_common::{Context, Round};

use super::Line;

#[derive_where(Clone, Debug, Eq, PartialEq)]
#[derive(Display)]
#[displaydoc("[{time}] height: {height}, round: {round}, line: {line}")]
pub struct Trace<Ctx: Context> {
    pub time: OffsetDateTime,
    pub height: Ctx::Height,
    pub round: Round,
    pub line: Line,
}

impl<Ctx: Context> Trace<Ctx> {
    pub fn new(height: Ctx::Height, round: Round, line: Line) -> Self {
        Self {
            time: OffsetDateTime::now_utc(),
            height,
            round,
            line,
        }
    }
}

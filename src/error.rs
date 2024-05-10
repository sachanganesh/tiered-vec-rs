use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum TierError<T>
where
    T: Debug,
{
    #[error("tier is full and cannot be inserted into")]
    TierFullInsertionError(T),

    #[error("rank not in valid range and insertion would be disconnected from main entries")]
    TierDisconnectedEntryInsertionError(usize, T),

    #[error("tier is empty and no element can be removed")]
    TierEmptyError,

    #[error("the provided rank is out of bounds")]
    TierRankOutOfBoundsError(usize),
    //
    // #[error("tier is full and at least some elements cannot be inserted")]
    // TierMultipleInsertionError(Vec<T>),
}

#[derive(Debug, Error)]
pub enum TieredVectorError<T>
where
    T: Debug,
{
    #[error("rank not in valid range and insertion would be disconnected from main entries")]
    TieredVectorOutofBoundsInsertionError(usize, T),

    #[error("tiered vector is empty and no element can be removed")]
    TieredVectorEmptyError,

    #[error("the provided rank is out of bounds")]
    TieredVectorRankOutOfBoundsError(usize),
}

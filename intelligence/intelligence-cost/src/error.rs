use thiserror::Error;

pub type CostResult<T> = Result<T, CostError>;

#[derive(Debug, Error)]
pub enum CostError {
    #[error("budget exceeded")]
    BudgetExceeded,
    #[error("quota exceeded")]
    QuotaExceeded,
    #[error("cost error: {0}")]
    Other(String),
}

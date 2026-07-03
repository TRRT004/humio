use thiserror::Error;

/// All errors that can be produced by humio operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HumioError {
	/// The underlying enigo backend failed.
	#[error("Input backend error: {0}")]
	Backend(String),
	/// Mouse location could not be obtained.
	#[error("Mouse location query failed: {0}")]
	LocationQuery(String),
}

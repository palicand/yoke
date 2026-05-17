use crate::state::ModeHint;

#[derive(thiserror::Error, Debug)]
pub enum VolumeError {
    #[error("no QuadStick volume mounted")]
    NotPresent,
    #[error("device visible but volume hidden: {hint:?}")]
    VolumeHidden { hint: Option<ModeHint> },
    #[error("invalid profile name: {0}")]
    InvalidProfileName(String),
    #[error("backend init failed: {0}")]
    BackendInit(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

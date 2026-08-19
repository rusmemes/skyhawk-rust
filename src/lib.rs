pub mod api;
pub mod config;
pub mod discovery;
pub mod domain;
pub mod error;
pub mod kafka;
pub mod services;
pub mod shutdown;
pub mod state;
pub mod storage;

pub use config::Config;
pub use error::AppError;
pub use state::{FrontState, ServiceList};

pub const HEADER_SENDER: &str = "sender";

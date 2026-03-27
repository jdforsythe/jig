pub mod schema;
pub mod resolve;
pub mod validate;
pub mod migrate;
pub mod migration;

pub use schema::{
    ConfigSource, HookTrustTier, JigConfig, Persona, Profile, Template, TemplateRef,
};
pub use resolve::{resolve_config, ResolvedConfig};
pub use validate::{validate_layer, ConfigError};

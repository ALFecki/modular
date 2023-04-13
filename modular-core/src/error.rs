#[derive(Debug)]
pub enum ModuleError {
    UnknownMethod,
    Custom(CustomModuleError),
    Destroyed,
}

#[derive(Debug)]
pub struct CustomModuleError {
    pub code: i32,
    pub name: Option<String>,
    pub message: Option<String>,
}

pub enum SubscribeError {
    InvalidPattern(anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum RegistryError {
    #[error("module already exists")]
    AlreadyExists,
}

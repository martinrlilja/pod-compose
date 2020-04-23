use std::{collections::BTreeMap as Map, path::PathBuf};

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ImageId(pub String);

#[derive(Clone, Debug, Hash)]
pub struct ImageSpec {
    pub image_name: String,
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub target: Option<String>,
    pub build_args: Map<String, String>,
}

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ContainerId(pub String);

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ContainerName(pub String);

#[derive(Clone, Debug, Hash)]
pub struct Container {
    pub id: ContainerId,
    pub name: ContainerName,
    pub status: ContainerStatus,
    pub labels: Map<String, String>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ContainerStatus {
    Running,
    Exited,
    Unknown,
}

#[derive(Clone, Debug, Hash)]
pub struct ContainerSpec {
    pub container_name: ContainerName,
    pub service_name: String,
    pub image_name: String,
    pub labels: Map<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Composition {
    pub images: Vec<ImageSpec>,
    pub containers: Vec<ContainerSpec>,
}

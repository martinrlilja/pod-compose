use std::{collections::BTreeMap as Map, path::PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Composition {
    pub build_images: Vec<ImageBuildSpec>,
    pub pull_images: Vec<ImagePullSpec>,
    pub containers: Vec<ContainerSpec>,
}

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ImageId(pub String);

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ImageName(pub String);

#[derive(Clone, Debug, Hash)]
pub struct Image {
    pub id: ImageId,
    pub labels: Map<String, String>,
}

#[derive(Clone, Debug, Hash)]
pub struct ImageBuildSpec {
    pub name: ImageName,
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub target: Option<String>,
    pub build_args: Map<String, String>,
    pub labels: Map<String, String>,
}

#[derive(Clone, Debug, Hash)]
pub struct ImagePullSpec {
    pub name: ImageName,
}

#[derive(Copy, Clone, Debug, Hash)]
pub enum PullPolicy {
    IfNotPresent,
    Always,
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
    Configured,
    Running,
    Exited,
    Unknown,
}

#[derive(Clone, Debug, Hash)]
pub struct ContainerSpec {
    pub name: ContainerName,
    pub service_name: String,
    pub image_name: ImageName,
    pub labels: Map<String, String>,
}

use std::{collections::BTreeMap as Map, path::PathBuf};

#[derive(Clone, Debug, Hash)]
pub struct ImageId(pub String);

#[derive(Clone, Debug, Hash)]
pub struct ImageSpec {
    pub image_name: String,
    pub context: PathBuf,
    pub dockerfile: PathBuf,
    pub target: Option<String>,
    pub build_args: Map<String, String>,
}

#[derive(Clone, Debug, Hash)]
pub struct ContainerId(pub String);

#[derive(Clone, Debug, Hash)]
pub struct ContainerSpec {
    pub service_name: String,
    pub container_name: String,
    pub image_name: String,
}

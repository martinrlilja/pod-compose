use anyhow::Result;
use std::{collections::BTreeMap as Map, path::Path};

use crate::models::{
    Composition, Container, ContainerId, ContainerName, ContainerSpec, Image, ImageBuildSpec,
    ImageId, ImageName, PullPolicy,
};

/// A frontend that reads a container spec file such as `docker-compose.yml`.
pub trait ComposerFrontend {
    /// Reads a compose file at the given path and returns the composition.
    /// The composition contains all images, volumes and containers that needs
    /// to be created. It's the caller's responsiblity to find a compatible frontend.
    fn composition(&mut self, project_name: &str, compose_file_path: &Path) -> Result<Composition>;
}

/// A container backends talks directly to `podman` or `docker`.
pub trait ContainerBackend {
    fn get_image(&mut self, name: &ImageName) -> Result<Option<Image>>;

    fn pull_image(&mut self, name: &ImageName) -> Result<ImageId>;

    fn build_image(&mut self, spec: &ImageBuildSpec, pull_policy: PullPolicy) -> Result<ImageId>;

    fn list_containers(
        &mut self,
        labels: Vec<(&str, &str)>,
    ) -> Result<Map<ContainerName, Container>>;

    fn create_container(&mut self, spec: ContainerSpec) -> Result<ContainerId>;

    fn start_container(&mut self, name: &str) -> Result<ContainerId>;

    fn stop_container(&mut self, name: &str, timeout: u32) -> Result<ContainerId>;

    fn remove_container(&mut self, name: &str, remove_volumes: bool) -> Result<ContainerId>;
}

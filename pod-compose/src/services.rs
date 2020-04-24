use anyhow::Result;
use std::{collections::BTreeMap as Map, path::Path};

use crate::models::{Composition, Container, ContainerId, ContainerName, ContainerSpec, ImageId, ImageSpec};

pub trait ComposerFrontend {
    fn composition<P: AsRef<Path>>(
        &mut self,
        project_name: &str,
        compose_file_path: P,
    ) -> Result<Composition>;
}

pub trait ContainerBackend {
    fn image_exists(&mut self, name: &str) -> Result<bool>;

    fn build_image(&mut self, image_spec: ImageSpec) -> Result<ImageId>;

    fn list_containers(
        &mut self,
        labels: Vec<(&str, &str)>,
    ) -> Result<Map<ContainerName, Container>>;

    fn create_container(&mut self, container_spec: ContainerSpec) -> Result<ContainerId>;

    fn start_container(&mut self, name: &str) -> Result<ContainerId>;

    fn stop_container(&mut self, name: &str, timeout: u32) -> Result<ContainerId>;

    fn remove_container(&mut self, name: &str, remove_volumes: bool) -> Result<ContainerId>;
}

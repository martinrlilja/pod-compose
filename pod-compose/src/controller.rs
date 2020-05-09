use anyhow::{anyhow, Result};
use blake3;
use log::info;
use std::collections::{BTreeMap as Map, BTreeSet as Set};

use crate::{
    hasher::DigestHasher,
    models::{
        Composition, Container, ContainerId, ContainerName, ContainerSpec, ContainerStatus,
        PullPolicy,
    },
    services::ContainerBackend,
};

const LABEL_PROJECT: &str = "io.podman.compose.project";
const LABEL_SERVICE: &str = "io.podman.compose.service";
const LABEL_HASH: &str = "io.podman.compose.hash";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ContainerOperation {
    Create,
    Recreate,
    Start,
    Stop,
    Remove,
}

pub struct Controller {
    backend: Box<dyn ContainerBackend>,
    composition: Composition,
    containers: Map<ContainerName, Container>,
    project_name: String,
}

impl Controller {
    pub fn init<B, P>(project_name: P, backend: B, composition: Composition) -> Result<Controller>
    where
        B: 'static + ContainerBackend,
        P: Into<String>,
    {
        let project_name = project_name.into();
        let mut backend = Box::new(backend);
        let containers = backend.list_containers(vec![(LABEL_PROJECT, &project_name)])?;

        Ok(Controller {
            backend,
            composition,
            containers,
            project_name,
        })
    }

    pub fn pull_images(&mut self, pull_policy: PullPolicy) -> Result<()> {
        for image_spec in self.composition.pull_images.iter() {
            let image = self.backend.get_image(&image_spec.name)?;

            match (pull_policy, image) {
                (PullPolicy::IfNotPresent, None) | (PullPolicy::Always, _) => {
                    self.backend.pull_image(&image_spec.name)?;
                }
                _ => (),
            }
        }

        Ok(())
    }

    /// Finds containers with a project label that is the same as the current project.
    /// This is useful in situations where the user removes a service from the
    /// compose file but forgets to stop and remove the container.
    pub fn find_orphans(&mut self) -> Result<Vec<ContainerName>> {
        let services = self
            .composition
            .containers
            .iter()
            .map(|spec| spec.service_name.clone())
            .collect::<Set<_>>();

        info!("found services: {:?}", services);

        let orphans = self
            .containers
            .iter()
            .filter_map(|(container_name, container)| {
                let service = container.labels.get(LABEL_SERVICE);
                match service {
                    Some(service) if services.contains(service) => None,
                    _ => Some(container_name.clone()),
                }
            })
            .collect();

        info!("found orphans: {:?}", orphans);

        Ok(orphans)
    }

    pub fn start_containers_diff(&mut self) -> Result<Vec<(ContainerName, ContainerOperation)>> {
        let diff = self.composition.containers.iter().filter_map(|spec| {
            let mut hasher = blake3::Hasher::new();
            hasher.input(&spec);
            let spec_hash = hasher.finalize();

            let container = match self.containers.get(&spec.name) {
                Some(container) => container,
                None => return Some((spec.name.clone(), ContainerOperation::Create)),
            };

            let container_hash = container.labels.get(LABEL_HASH);
            if container_hash
                .map(|h| h == spec_hash.to_hex().as_str())
                .unwrap_or(false)
            {
                let operation = match container.status {
                    ContainerStatus::Configured => Some(ContainerOperation::Start),
                    ContainerStatus::Running => None,
                    ContainerStatus::Exited => Some(ContainerOperation::Start),
                    ContainerStatus::Unknown => Some(ContainerOperation::Recreate),
                };

                operation.map(|operation| (spec.name.clone(), operation))
            } else {
                Some((spec.name.clone(), ContainerOperation::Recreate))
            }
        });

        let services = self
            .composition
            .containers
            .iter()
            .map(|spec| spec.service_name.clone())
            .collect::<Set<_>>();

        // If the user scales down any service, we need to find the old
        // containers and remove them. Making sure we don't also remove
        // orphans.
        let scaled_down_containers =
            self.containers
                .iter()
                .filter_map(|(container_name, container)| {
                    let container_should_exist = self
                        .composition
                        .containers
                        .iter()
                        .find(|container_spec| container_spec.name == *container_name)
                        .is_some();

                    let service = container.labels.get(LABEL_SERVICE);
                    match service {
                        Some(service) if services.contains(service) && !container_should_exist => {
                            Some((container_name.clone(), ContainerOperation::Remove))
                        }
                        _ => None,
                    }
                });

        let diff = diff.chain(scaled_down_containers).collect();

        Ok(diff)
    }

    pub fn stop_containers_diff(&mut self) -> Result<Vec<(ContainerName, ContainerOperation)>> {
        let diff = self
            .composition
            .containers
            .iter()
            .filter_map(|spec| match self.containers.get(&spec.name) {
                Some(container) if container.status == ContainerStatus::Running => {
                    Some((spec.name.clone(), ContainerOperation::Stop))
                }
                _ => None,
            })
            .collect();

        Ok(diff)
    }

    pub fn remove_containers_diff(&mut self) -> Result<Vec<(ContainerName, ContainerOperation)>> {
        let diff = self
            .composition
            .containers
            .iter()
            .filter_map(|spec| match self.containers.get(&spec.name) {
                Some(_container) => Some((spec.name.clone(), ContainerOperation::Remove)),
                _ => None,
            })
            .collect();

        Ok(diff)
    }

    pub fn container_apply(
        &mut self,
        name: &ContainerName,
        operation: ContainerOperation,
        timeout: u32,
    ) -> Result<()> {
        let container_spec = || -> Result<ContainerSpec> {
            self.composition
                .containers
                .iter()
                .find(|spec| spec.name == *name)
                .ok_or_else(|| anyhow!("unknown container name: {:?}", name))
                .map(|spec| spec.clone())
        };

        match operation {
            ContainerOperation::Create => {
                let container_spec = container_spec()?;
                let container_id = self.container_create(container_spec)?;
                self.backend.start_container(&container_id.0)?;
            }
            ContainerOperation::Recreate => {
                let container_spec = container_spec()?;
                let container = self
                    .containers
                    .get(name)
                    .ok_or_else(|| anyhow!("could not find container {:?}", name))?;

                if container.status == ContainerStatus::Running {
                    self.backend.stop_container(&container.id.0, timeout)?;
                }
                self.backend.remove_container(&container.id.0, false)?;
                let container_id = self.container_create(container_spec)?;
                self.backend.start_container(&container_id.0)?;
            }
            ContainerOperation::Start => {
                let container = self
                    .containers
                    .get(name)
                    .ok_or_else(|| anyhow!("could not find container {:?}", name))?;
                self.backend.start_container(&container.id.0)?;
            }
            ContainerOperation::Stop => {
                let container = self
                    .containers
                    .get(name)
                    .ok_or_else(|| anyhow!("could not find container {:?}", name))?;
                self.backend.stop_container(&container.id.0, timeout)?;
            }
            ContainerOperation::Remove => {
                let container = self
                    .containers
                    .get(name)
                    .ok_or_else(|| anyhow!("could not find container {:?}", name))?;

                if container.status == ContainerStatus::Running {
                    self.backend.stop_container(&container.id.0, timeout)?;
                }
                self.backend.remove_container(&container.id.0, false)?;
            }
        }

        Ok(())
    }

    pub fn container_create(&mut self, mut spec: ContainerSpec) -> Result<ContainerId> {
        let mut hasher = blake3::Hasher::new();
        hasher.input(&spec);
        let hash = hasher.finalize();

        spec.labels
            .insert(LABEL_PROJECT.into(), self.project_name.clone());
        spec.labels
            .insert(LABEL_SERVICE.into(), spec.service_name.clone());
        spec.labels
            .insert(LABEL_HASH.into(), hash.to_hex().to_string());

        let id = self.backend.create_container(spec)?;

        Ok(id)
    }
}

use anyhow::Result;
use serde_yaml;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

use crate::docker_compose::{Build, DockerComposeFile};
use crate::models::{ContainerName, ContainerSpec, ImageSpec};

#[derive(Clone, Debug, Default)]
pub struct Composition {
    pub images: Vec<ImageSpec>,
    pub containers: Vec<ContainerSpec>,
}

pub trait ComposerFrontend {
    fn composition<P: AsRef<Path>>(
        &mut self,
        project_name: &str,
        compose_file_path: P,
    ) -> Result<Composition>;
}

pub struct DockerComposeFrontend;

impl DockerComposeFrontend {
    pub fn new() -> DockerComposeFrontend {
        DockerComposeFrontend
    }
}

impl ComposerFrontend for DockerComposeFrontend {
    fn composition<P: AsRef<Path>>(
        &mut self,
        project_name: &str,
        compose_file_path: P,
    ) -> Result<Composition> {
        let compose_file_path = compose_file_path.as_ref();
        let compose_file = File::open(&compose_file_path)?;

        let file: DockerComposeFile = serde_yaml::from_reader(compose_file)?;
        let mut composition: Composition = Default::default();

        for (service_name, service) in file.services {
            let image_name = match service.image {
                Some(image_name) => image_name,
                None => format!("{}_{}", project_name, service_name),
            };

            match service.build {
                Some(Build::Short(context)) => {
                    let image_spec = ImageSpec {
                        context: PathBuf::from(context),
                        dockerfile: PathBuf::from("Dockerfile"),
                        target: None,
                        build_args: Default::default(),
                        image_name: image_name.clone(),
                    };
                    composition.images.push(image_spec);
                }
                Some(Build::Extended {
                    context,
                    dockerfile,
                    args,
                    target,
                    ..
                }) => {
                    let image_spec = ImageSpec {
                        context: PathBuf::from(context),
                        dockerfile: PathBuf::from(
                            dockerfile.unwrap_or_else(|| "Dockerfile".into()),
                        ),
                        target: target,
                        build_args: args.to_map(),
                        image_name: image_name.clone(),
                    };
                    composition.images.push(image_spec);
                }
                None => (),
            }

            for index in 0..service.replicas.unwrap_or(1) {
                let container = ContainerSpec {
                    service_name: service_name.clone(),
                    image_name: image_name.clone(),
                    container_name: ContainerName(format!("{}_{}_{}", project_name, service_name, index)),
                    labels: Default::default(),
                };
                composition.containers.push(container);
            }
        }

        Ok(composition)
    }
}

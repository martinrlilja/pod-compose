use anyhow::{anyhow, Result};
use blake3;
use std::{
    env,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

use composer_frontend::{ComposerFrontend, DockerComposeFrontend};
use container_backend::{ContainerBackend, PodmanBackend};
use hasher::DigestHasher;
use models::ContainerStatus;

mod composer_frontend;
mod container_backend;
mod docker_compose;
mod hasher;
mod models;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "pod",
    about = "A docker-compose compatible tool for running containers with podman."
)]
enum Opt {
    Build {
        #[structopt(short, long)]
        pull: bool,
    },
    Down {
        #[structopt(short, long)]
        /// Also remove containers.
        volumes: bool,

        #[structopt(long, default_value = "5")]
        timeout: u32,
    },
    /// Finds a docker-compose.yaml file and starts the containers defined in it.
    Up {
        #[structopt(short, long)]
        /// Start containers in the background.
        detach: bool,

        #[structopt(long)]
        /// Build images before starting the containers.
        build: bool,
    },
    Stop {
        #[structopt(long, default_value = "5")]
        timeout: u32,
    },
}

fn find_compose_file<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    for path in path.as_ref().ancestors() {
        let docker_file_path = path.join("docker-compose.yml");
        if docker_file_path.exists() {
            return Some(docker_file_path);
        }

        let docker_file_path = path.join("docker-compose.yaml");
        if docker_file_path.exists() {
            return Some(docker_file_path);
        }
    }

    None
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let current_dir = env::current_dir()?;
    let compose_file_path = find_compose_file(current_dir);

    let compose_file_path = compose_file_path
        .ok_or_else(|| anyhow!("Couldn't find a docker-compose.yml file in the current working directory or any of its parents."))?;

    let work_directory = compose_file_path
        .parent()
        .ok_or_else(|| anyhow!("Docker compose file has no parent."))?;

    env::set_current_dir(work_directory)?;

    let project_name = work_directory
        .file_name()
        .and_then(|path| path.to_str())
        .ok_or_else(|| anyhow!("Couldn't determine the project name."))?;

    let mut frontend = DockerComposeFrontend::new();
    let composition = frontend.composition(project_name, &compose_file_path)?;

    let mut backend = PodmanBackend::connect()?;

    let containers = backend.list_containers(vec![("io.podman.compose.project", &project_name)])?;

    match opt {
        Opt::Build { pull: _ } => (),
        Opt::Down {
            volumes: _,
            timeout: _,
        } => (),
        Opt::Up { detach: _, build } => {
            for image_spec in composition.images {
                if !build && backend.image_exists(&image_spec.image_name)? {
                    continue;
                }

                println!("Building image {}", image_spec.image_name);
                backend.build_image(image_spec)?;
            }

            for mut container_spec in composition.containers {
                let container_name = container_spec.container_name.clone();
                let current_container = containers.get(&container_name);

                let mut hasher = blake3::Hasher::new();
                hasher.input(&container_spec);
                let hash = hasher.finalize();

                container_spec.labels.insert("io.podman.compose.project".into(), project_name.into());
                container_spec.labels.insert("io.podman.compose.service".into(), container_spec.service_name.clone());
                container_spec.labels.insert("io.podman.compose.hash".into(), hash.to_hex().to_string());

                match current_container {
                    Some(container) => match container.status {
                        ContainerStatus::Running => (),
                        ContainerStatus::Exited => {
                            println!("Starting container {}", container_name.0);
                            backend.start_container(&container.id.0)?;
                        }
                        ContainerStatus::Unknown => {
                            println!("Recreating container {}", container_name.0);
                            backend.remove_container(&container.id.0, false)?;
                            let container_id = backend.create_container(container_spec)?;
                            backend.start_container(&container_id.0)?;
                        }
                    },
                    None => {
                        println!("Creating container {}", container_name.0);
                        let container_id = backend.create_container(container_spec)?;
                        backend.start_container(&container_id.0)?;
                    }
                }
            }
        }
        Opt::Stop { timeout } => {
            for container_spec in composition.containers {
                let current_container = containers.get(&container_spec.container_name);

                match current_container {
                    Some(container) if container.status == ContainerStatus::Running => {
                        println!("Stopping container {}...", &container_spec.container_name.0);
                        backend.stop_container(&container.id.0, timeout)?;
                    }
                    _ => (),
                }
            }
        }
    }

    Ok(())
}

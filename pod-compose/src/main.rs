use anyhow::{anyhow, Result};
use crossterm::{
    cursor,
    style::{self, Colorize, Styler},
    QueueableCommand,
};
use log::info;
use std::{
    env,
    io::{stdout, Write},
    path::{Path, PathBuf},
};
use structopt::StructOpt;

use backends::PodmanBackend;
use controller::{ContainerOperation, Controller};
use frontends::DockerComposeFrontend;
use models::{BuildPolicy, ContainerName, PullPolicy};
use services::ComposerFrontend;

mod backends;
mod controller;
mod frontends;
mod hasher;
mod models;
mod services;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "pod-compose",
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

        #[structopt(long)]
        remove_orphans: bool,
    },
    /// Finds a docker-compose.yaml file and starts the containers defined in it.
    Up {
        #[structopt(short, long)]
        /// Start containers in the background.
        detach: bool,

        #[structopt(long)]
        /// Build images before starting the containers.
        build: bool,

        #[structopt(long, default_value = "5")]
        timeout: u32,

        #[structopt(long)]
        remove_orphans: bool,
    },
    Stop {
        #[structopt(long, default_value = "5")]
        timeout: u32,

        #[structopt(long)]
        remove_orphans: bool,
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
    pretty_env_logger::init_custom_env("LOG");

    let opt = Opt::from_args();

    let mut stdout = stdout();

    let current_dir = env::current_dir()?;
    let compose_file_path = find_compose_file(current_dir);

    let compose_file_path = compose_file_path
        .ok_or_else(|| anyhow!("Couldn't find a docker-compose.yml file in the current working directory or any of its parents."))?;
    info!("found compose file {:?}", compose_file_path);

    let work_directory = compose_file_path
        .parent()
        .ok_or_else(|| anyhow!("Docker compose file has no parent."))?;
    info!("found work directory {:?}", work_directory);

    env::set_current_dir(work_directory)?;

    let project_name = work_directory
        .file_name()
        .and_then(|path| path.to_str())
        .ok_or_else(|| anyhow!("Couldn't determine the project name."))?;
    info!("project name {:?}", project_name);

    let mut frontend = DockerComposeFrontend::new();
    let composition = frontend.composition(project_name, compose_file_path.as_path())?;
    info!("parsed composition");

    let backend = PodmanBackend::connect()?;
    info!("connected to podman");

    let mut controller = Controller::init(project_name, backend, composition)?;
    info!("created controller");

    match opt {
        Opt::Build { pull } => {
            let pull_policy = if pull {
                PullPolicy::Always
            } else {
                PullPolicy::IfNotPresent
            };

            controller.build_images(BuildPolicy::Always, pull_policy)?;
        }
        Opt::Down {
            volumes: _,
            timeout,
            remove_orphans,
        } => {
            check_orphans(&mut controller, &mut stdout, remove_orphans, timeout)?;

            let diff = controller.remove_containers_diff()?;
            container_apply(&mut controller, &mut stdout, diff, timeout)?;
        }
        Opt::Up {
            detach: _,
            build,
            timeout,
            remove_orphans,
        } => {
            check_orphans(&mut controller, &mut stdout, remove_orphans, timeout)?;

            controller.pull_images(PullPolicy::IfNotPresent)?;

            let build_policy = if build {
                BuildPolicy::Always
            } else {
                BuildPolicy::IfChanged
            };

            controller.build_images(build_policy, PullPolicy::IfNotPresent)?;

            let diff = controller.start_containers_diff()?;
            container_apply(&mut controller, &mut stdout, diff, timeout)?;
        }
        Opt::Stop {
            timeout,
            remove_orphans,
        } => {
            check_orphans(&mut controller, &mut stdout, remove_orphans, timeout)?;

            let diff = controller.stop_containers_diff()?;
            container_apply(&mut controller, &mut stdout, diff, timeout)?;
        }
    }

    Ok(())
}

/// Looks for orphans, if there are any and `remove_orphans` is set to true
/// they will be removed. Otherwise a message will be printed.
fn check_orphans(
    controller: &mut Controller,
    stdout: &mut impl Write,
    remove_orphans: bool,
    timeout: u32,
) -> Result<()> {
    let orphans = controller.find_orphans()?;

    if !orphans.is_empty() {
        if remove_orphans {
            let diff = orphans
                .into_iter()
                .map(|name| (name, ContainerOperation::Remove))
                .collect();
            container_apply(controller, stdout, diff, timeout)?;
        } else {
            stdout
                .queue(style::PrintStyledContent("INFO: ".cyan().bold()))?
                .queue(style::Print(
                    "found orphans, rerun with --remove-orphans to remove them.\n",
                ))?
                .flush()?;
        }
    } else {
        info!("found no orphans");
    }

    Ok(())
}

fn container_apply(
    controller: &mut Controller,
    stdout: &mut impl Write,
    operations: Vec<(ContainerName, ContainerOperation)>,
    timeout: u32,
) -> Result<()> {
    fn operation_verb(operation: ContainerOperation) -> &'static str {
        match operation {
            ContainerOperation::Create => "Creating",
            ContainerOperation::Recreate => "Recreating",
            ContainerOperation::Start => "Starting",
            ContainerOperation::Stop => "Stopping",
            ContainerOperation::Remove => "Removing",
        }
    }

    let lines = operations
        .iter()
        .map(|(container_name, operation)| {
            let verb = operation_verb(*operation);
            format!("{} {}", verb, container_name.0)
        })
        .collect::<Vec<_>>();

    let longest_line = lines.iter().map(|line| line.len()).max().unwrap_or(0);

    for line in lines.iter() {
        stdout.queue(style::Print(line))?;

        let padding = longest_line - line.len() + 1;
        stdout
            .queue(cursor::MoveRight(padding as u16))?
            .queue(style::Print("...\n"))?;
    }

    stdout.flush()?;

    for (line, (container_name, operation)) in operations.into_iter().enumerate() {
        controller.container_apply(&container_name, operation, timeout)?;

        stdout
            .queue(cursor::SavePosition)?
            .queue(cursor::MoveToPreviousLine((lines.len() - line) as u16))?
            .queue(cursor::MoveRight(longest_line as u16 + 5))?
            .queue(style::PrintStyledContent("done".green().bold()))?
            .queue(cursor::RestorePosition)?
            .flush()?;
    }

    Ok(())
}

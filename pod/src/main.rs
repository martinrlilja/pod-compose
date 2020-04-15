use anyhow::Result;
use podman_varlink::{VarlinkClient, VarlinkClientInterface};
use structopt::StructOpt;
use varlink::Connection;

#[derive(Debug, StructOpt)]
#[structopt(name = "pod", about = "A docker-compose compatible tool for running containers with podman.")]
enum Opt {
    /// Finds a docker-compose.yaml file and starts the containers defined in it.
    Up {
        #[structopt(short, long)]
        /// Start containers in the background.
        detach: bool,

        #[structopt(short, long)]
        /// Build images before starting the containers.
        build: bool,
    },
    Down {
        #[structopt(short, long)]
        /// Also remove containers.
        volumes: bool,
    },
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    let connection = Connection::with_bridge("podman varlink --timeout=0")?;
    let mut podman = VarlinkClient::new(connection.clone());
    let version = podman.get_version().call()?;

    println!("{:?}", version);

    Ok(())
}

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use sprinkler_api::*;
mod docker_oom;
use docker_oom::DockerOOM;
mod config;

fn main() {
    let args = clap_app!(sprinkler =>
            (version: crate_version!())
            (author: crate_authors!())
            (about: crate_description!())
            (@arg VERBOSE: --verbose -v ... "Logging verbosity")
        ).get_matches();
    
    config::setup_logger(args.occurrences_of("VERBOSE")).expect("Logger Error.");

    const MASTER_ADDR: &str = "bridge.dsa.lan:3777";
    let mut builder = SprinklerBuilder::new(SprinklerOptions{ master_addr: String::from(MASTER_ADDR), ..Default::default() });

    // parse FNAME_CONFIG and add triggers
    let sprinklers: Vec<Box<dyn Sprinkler>> = vec![
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cp-1"))),
        // Box::new(builder.build::<DockerOOM>(String::from("alex-jetson-tx2")))
    ];

    let switch = Switch::new();
    switch.connect_all(&sprinklers);
    let addr = MASTER_ADDR.parse().unwrap();
    sprinkler_api::server(&addr, &switch);
}

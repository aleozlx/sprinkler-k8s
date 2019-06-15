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

    const MASTER_ADDR: &str = "192.168.0.3:3777";
    let mut builder = SprinklerBuilder::new(SprinklerOptions{ master_addr: String::from(MASTER_ADDR), ..Default::default() });

    // parse FNAME_CONFIG and add triggers
    let sprinklers: Vec<Box<dyn Sprinkler>> = vec![
        Box::new(builder.build::<CommCheck>(String::from("alex-jetson-tx2"))),
        Box::new(builder.build::<DockerOOM>(String::from("alex-jetson-tx2")))
    ];

    #[cfg(not(feature = "master"))] {
        sprinkler_api::agent(&sprinklers);
        sprinkler_api::loop_forever();
    }
    #[cfg(feature = "master")] {
        let switch = Switch::new();
        switch.connect_all(&sprinklers);
        let addr = "0.0.0.0:3777".parse().unwrap();
        sprinkler_api::server(&addr, &switch);
    }
}

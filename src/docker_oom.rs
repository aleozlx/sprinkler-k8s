use std::io::Write;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::collections::HashMap;
use std::borrow::Cow;
use tokio::prelude::*;
use super::*;

#[derive(Clone)]
pub struct DockerOOM {
    options: Arc<SprinklerOptions>,
    _deactivate: Arc<Mutex<bool>>
}

pub trait ImportantExt {
    fn is_important(&self) -> bool;
}

pub struct EventRateMeter {
    count: usize,                        // Event counter
    t0: chrono::DateTime<chrono::Local>, // Starting point of this period
    last_rate: f32,                      // Last reading
    interval: chrono::Duration,          // Measurement interval
    pub state: Anomaly                   // aka Is this (ab)normal?
}

impl Default for EventRateMeter {
    fn default() -> Self {
        EventRateMeter {
            count: 0,
            t0: chrono::Local::now(),
            last_rate: 0f32,
            interval: chrono::Duration::seconds(1),
            state: Anomaly::Negative
        }
    }
}

impl EventRateMeter {
    /// Trigger the meter counter
    pub fn tick(&mut self) {
        self.count += 1;
        if chrono::Local::now() - self.t0 > self.interval {
            self.last_rate = self.read();
            self.count = 0;
        }
    }

    fn dt(&self) -> f32 {
        (self.interval.num_milliseconds() as f32) / 1e3
    }

    /// Compute event frequency (Hz)
    pub fn read(&self) -> f32 {
        if self.count < 6 { self.last_rate }
        else { (self.count as f32) / self.dt() }
    }
}

#[derive(Default)]
struct FrequencyDivider {
    count: usize,             // Event counter
    interval: usize,          // Reset interval
}

impl FrequencyDivider {
    /// Trigger the meter counter
    pub fn tick(&mut self) {
        self.count += 1;
        if self.count == self.interval {
            self.count = 0;
        }
    }

    /// Output event
    pub fn read(&self) -> bool {
        self.count == 0
    }
}

impl ImportantExt for AnomalyTransition {
    fn is_important(&self) -> bool {
        match self {
            AnomalyTransition::Occurred | AnomalyTransition::Disappeared | AnomalyTransition::Fixed | AnomalyTransition::GaveUp => true,
            _ => false
        }
    }
}

type MeterSet = Arc<RwLock<HashMap<String, Mutex<(EventRateMeter, FrequencyDivider)>>>>;

impl Sprinkler for DockerOOM {
    fn build(options: SprinklerOptions) -> Self {
        DockerOOM {
            options: Arc::new(options),
            _deactivate: Arc::new(Mutex::new(false))
        }
    }

    fn id(&self) -> usize {
        self.options._id
    }

    fn hostname(&self) -> &str {
        &self.options._hostname
    }

    fn activate_master(&self) -> ActivationResult {
        let (tx, rx) = futures::sync::mpsc::channel::<Message>(512);
        tokio::run({
            rx.for_each({ let clone = self.clone(); move |message| {
                info!(
                    "sprinkler[{}] (DockerOOM) {}:\n{}",
                    clone.id(), clone.hostname(), &message.body
                );
                Ok(())
            }})
        });
        ActivationResult::AsyncMonitor(tx)
    }

    fn activate_agent(&self) {
        let clone = self.clone();
        let docker = shiplift::Docker::new();
        let mut meters = HashMap::new();
        meters.insert(String::from("!"), Default::default()); // Other types of message flooding
        meters.insert(String::from("."), Mutex::new((
            Default::default(), // Unidentified OOM
            FrequencyDivider { interval: 15, ..Default::default() }
        )));
        let meters: MeterSet = Arc::new(RwLock::new(meters));
        let monitor = docker
            .events(&Default::default()) // Stream and filter all docker events
            .for_each({ let meters = meters.clone(); move |e| {
                if e.typ == "container" && e.action == "oom" {
                    if let Some(pod_name) = e.actor.attributes.get("io.kubernetes.pod.name") {
                        clone.handle_anticipated_oom(meters.clone(), pod_name, &e.actor);
                    }
                    else { // The container is not managed by Kubernetes
                        clone.handle_other_oom(meters.clone(), &e.actor);
                    }
                }
                else { clone.handle_other_panic(meters.clone()); }
                Ok(())
            }})
            .map_err(|e| error!("{}", e));
        tokio::run(monitor);
    }

    fn deactivate(&self) {
        *self._deactivate.lock().unwrap() = true;
    }
}

impl DockerOOM {
    fn handle_anticipated_oom<'a>(&self, meters: MeterSet, pod_name: &'a str, actor: &'a shiplift::rep::Actor) {
        let need_new_meter = !meters.read().unwrap().contains_key(pod_name);
        if need_new_meter {
            let meter = (
                EventRateMeter { count: 1, state: Anomaly::Fixing(1), ..Default::default() }, // Jump to fixing(1) state
                FrequencyDivider { interval: 15, ..Default::default() } // Divide event frequency by 15
            );
            meters.write().unwrap().insert(String::from(pod_name), Mutex::new(meter));
        }
        else {
            let meters = meters.read().unwrap();
            let mut meter = meters[pod_name].lock().unwrap();
            meter.0.tick();
            if meter.0.read() > 10.0 { // Event rate > 10 Hz
                let transition = meter.0.state.escalate(20); // 20 retries till declaring out-of-control
                meter.1.tick();
                if meter.1.read() { // Hit handling schedule
                    if transition == AnomalyTransition::Fixing {
                        self.fix_it(actor.id.clone());
                    }
                    if transition.is_important() {
                        // Reachable states: Positive, Fixing(n), Out-of-control
                        // Reachable transitions: Occurred, Giveup
                        let mut data_ = HashMap::new();
                        data_.insert(String::from("msg"), format!("DockerOOM {:?}", &transition));
                        data_.insert(
                            String::from("io.kubernetes.pod.namespace"),
                            actor.attributes.get("io.kubernetes.pod.namespace").unwrap().clone());
                        data_.insert(
                            String::from("io.kubernetes.pod.name"),
                            actor.attributes.get("io.kubernetes.pod.name").unwrap().clone());
                        data_.insert(
                            String::from("io.kubernetes.pod.uid"),
                            actor.attributes.get("io.kubernetes.pod.uid").unwrap().clone());
                        Notification{
                            from: self.id(),
                            to_addr: self.options.master_addr.clone(),
                            data: data_
                        }.send(self.options.master_addr.clone());
                    }
                    meter.0.state >>= transition;
                }
            }
            else {
                let transition = meter.0.state.diminish();
                let mut data_ = HashMap::new();
                data_.insert(String::from("msg"), format!("DockerOOM {:?}", &transition));
                data_.insert(
                    String::from("io.kubernetes.pod.namespace"),
                    actor.attributes.get("io.kubernetes.pod.namespace").unwrap().clone());
                data_.insert(
                    String::from("io.kubernetes.pod.name"),
                    actor.attributes.get("io.kubernetes.pod.name").unwrap().clone());
                data_.insert(
                    String::from("io.kubernetes.pod.uid"),
                    actor.attributes.get("io.kubernetes.pod.uid").unwrap().clone());
                match transition {
                    AnomalyTransition::Disappeared => {
                        Notification{
                            from: self.id(),
                            to_addr: self.options.master_addr.clone(),
                            data: data_
                        }.send(self.options.master_addr.clone());
                    }
                    AnomalyTransition::Fixed => {
                        Notification{
                            from: self.id(),
                            to_addr: self.options.master_addr.clone(),
                            data: data_
                        }.send(self.options.master_addr.clone());
                    }
                    _ => {}
                }
                meter.0.state >>= transition;
            }
        }
    }

    fn handle_other_oom<'a>(&self, meters: MeterSet, actor: &'a shiplift::rep::Actor) {
        let meters = meters.read().unwrap();
        let mut meter = meters["."].lock().unwrap();
        meter.0.tick();
        if meter.0.read() > 10.0 {
            let transition = meter.0.state.escalate(20);
            meter.1.tick();
            if meter.1.read() {
                if transition == AnomalyTransition::Fixing {
                    self.fix_it(actor.id.clone());
                }
                if transition.is_important() {
                    // Reachable states: Positive, Fixing(n), Out-of-control
                    // Reachable transitions: Occurred, Giveup
                    let mut data_ = HashMap::new();
                    data_.insert(String::from("msg"), format!("DockerOOM {:?}", &transition));
                    data_.insert(
                        String::from("name"),
                        actor.attributes.get("name").unwrap().clone());
                    Notification{
                        from: self.id(),
                        to_addr: self.options.master_addr.clone(),
                        data: data_
                    }.send(self.options.master_addr.clone());
                }
                meter.0.state >>= transition;
            }
        }
        else {
            let transition = meter.0.state.diminish();
            if transition.is_important() {
                // Reachable states: Negative
                // Reachable transitions: Fixed, Disappeared
                let mut data_ = HashMap::new();
                data_.insert(String::from("msg"), format!("DockerOOM {:?}", &transition));
                data_.insert(
                    String::from("name"),
                    actor.attributes.get("name").unwrap().clone());
                Notification{
                    from: self.id(),
                    to_addr: self.options.master_addr.clone(),
                    data: data_
                }.send(self.options.master_addr.clone());
            }
            meter.0.state >>= transition;
        }
    }

    fn handle_other_panic(&self, meters: MeterSet) {
        let meters = meters.read().unwrap();
        let mut meter = meters["."].lock().unwrap();
        meter.0.tick();
        if meter.0.read() > 70.0 {
            if let Some(transition) = meter.0.state >> Anomaly::Positive {
                if transition.is_important() {
                    // Reachable states: Positive, Fixing(n), Out-of-control
                    // Reachable transitions: Occurred, Giveup
                    let mut data_ = HashMap::new();
                    data_.insert(String::from("msg"), format!("Docker Panic"));
                    Notification{
                        from: self.id(),
                        to_addr: self.options.master_addr.clone(),
                        data: data_
                    }.send(self.options.master_addr.clone());
                }
                meter.0.state = Anomaly::Positive;
            }
        }
        else {
            let transition = meter.0.state.diminish();
            if transition.is_important() {
                // Reachable states: Negative
                // Reachable transitions: Fixed, Disappeared
                let mut data_ = HashMap::new();
                data_.insert(String::from("msg"), format!("Docker Panic"));
                Notification{
                    from: self.id(),
                    to_addr: self.options.master_addr.clone(),
                    data: data_
                }.send(self.options.master_addr.clone());
            }
            meter.0.state >>= transition;
        }
    }

    fn fix_it(&self, id: String) {
        let docker = shiplift::Docker::new();
        let container = shiplift::Container::new(&docker, &id);
        let fut_kill = container.kill(None)  // Should send SIGKILL by default
            .map_err({ let id = id.clone(); move |_| {
                error!("Unable to kill a contianer: {}", &id);
            }})
            .and_then({
                let container_id = id.clone();
                let sprinkler_id = self.id();
                let master_addr = self.options.master_addr.clone();
                move |_| {
                    let mut data_ = HashMap::new();
                    data_.insert(String::from("msg"), format!("Killed {}", &container_id));
                    Notification {
                        from: sprinkler_id,
                        to_addr: master_addr,
                        data: data_
                    }
                }
            });
        tokio::spawn(fut_kill);
    }
}

const RETRY_DELAY: u64 = 20;

/// An asynchronous message
#[derive(Clone, Debug)]
struct Notification {
    /// Raw data in the notification
    /// Fields: message, io.kubernetes.pod.{namespace,name,uid}
    data: HashMap<String, String>,

    from: usize,
    to_addr: String,
}

impl Future for Notification {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Ok(socket) = std::net::TcpStream::connect(&self.to_addr) {
            let mut tlsbuilder = native_tls::TlsConnector::builder();
            tlsbuilder.add_root_certificate(native_tls::Certificate::from_pem(include_bytes!("/etc/sprinkler.conf.d/master.crt")).unwrap());
            let connector = tlsbuilder.build().expect("failed to build a TLS connector");
            let mut stream = connector.connect(&&self.to_addr.split(":").take(1).collect::<Vec<&str>>()[0], socket).expect("failed to establish a TLS stream");
            let serialized = String::from(self.data.iter().map(|(k, v)| format!("{} = {}", k, v)).collect::<Vec<String>>().join("\n"));
            let buf = super::compose_message(self.from, serialized);
            if let Err(e) = stream.write_all(&buf) {
                debug!("Failed to send the master thread a message: {}", e);
                thread::sleep(std::time::Duration::from_secs(RETRY_DELAY));
                Ok(Async::NotReady)
            }
            else {
                Ok(Async::Ready(()))
            }
        }
        else {
            debug!("Connection error, will retry after {} seconds.", RETRY_DELAY);
            thread::sleep(std::time::Duration::from_secs(RETRY_DELAY));
            Ok(Async::NotReady)
        }
    }
}

impl Notification {
    pub fn send(self, master_addr: String) {
        tokio::spawn(self);
    }
}

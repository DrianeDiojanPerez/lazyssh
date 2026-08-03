use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::models::{Reachability, SshHost};

/// How long a host is given to answer before it counts as unreachable.
const TIMEOUT: Duration = Duration::from_secs(2);

/// How many hosts are tried at once, so a long config does not open a thread
/// for every line in it.
const BATCH: usize = 8;

/// Knocks on each host's ssh port in the background and remembers who answered.
/// A connection that opens is the honest test of "can I ssh there", which
/// ICMP ping is not.
#[derive(Clone, Default)]
pub struct Probes {
    states: Arc<Mutex<HashMap<String, Reachability>>>,
    in_flight: Arc<AtomicUsize>,
}

impl Probes {
    pub fn status(&self, alias: &str) -> Reachability {
        self.states
            .lock()
            .map(|states| states.get(alias).copied().unwrap_or(Reachability::Unknown))
            .unwrap_or(Reachability::Unknown)
    }

    /// True while any host is still being tried, which is what keeps the
    /// interface redrawing until the last dot has settled.
    pub fn is_working(&self) -> bool {
        self.in_flight.load(Ordering::Relaxed) > 0
    }

    pub fn check_all(&self, hosts: &[SshHost]) {
        let targets: Vec<(String, String, u16)> = hosts
            .iter()
            .filter(|host| !host.display_host().is_empty())
            .map(|host| (host.alias.clone(), host.display_host().to_string(), host.port))
            .collect();

        if targets.is_empty() {
            return;
        }

        if let Ok(mut states) = self.states.lock() {
            for (alias, _, _) in &targets {
                states.insert(alias.clone(), Reachability::Checking);
            }
        }

        let states = Arc::clone(&self.states);
        let in_flight = Arc::clone(&self.in_flight);
        in_flight.fetch_add(1, Ordering::Relaxed);

        std::thread::spawn(move || {
            for batch in targets.chunks(BATCH) {
                let workers: Vec<_> = batch
                    .iter()
                    .cloned()
                    .map(|(alias, host, port)| {
                        std::thread::spawn(move || (alias, reach(&host, port)))
                    })
                    .collect();

                for worker in workers {
                    if let Ok((alias, status)) = worker.join() {
                        if let Ok(mut states) = states.lock() {
                            states.insert(alias, status);
                        }
                    }
                }
            }

            in_flight.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

fn reach(host: &str, port: u16) -> Reachability {
    let Ok(addresses) = (host, port).to_socket_addrs() else {
        return Reachability::Offline;
    };

    for address in addresses {
        if TcpStream::connect_timeout(&address, TIMEOUT).is_ok() {
            return Reachability::Online;
        }
    }

    Reachability::Offline
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

    fn host(alias: &str, hostname: &str, port: u16) -> SshHost {
        SshHost {
            alias: alias.into(),
            hostname: hostname.into(),
            port,
            ..SshHost::empty()
        }
    }

    #[test]
    fn a_port_that_answers_comes_back_online() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let probes = Probes::default();
        probes.check_all(&[host("up", "127.0.0.1", port)]);

        let deadline = Instant::now() + Duration::from_secs(5);
        while probes.status("up") != Reachability::Online {
            assert!(Instant::now() < deadline, "the open port was never reached");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_name_that_does_not_resolve_comes_back_offline() {
        let probes = Probes::default();
        probes.check_all(&[host("down", "no-such-host.invalid", 22)]);

        let deadline = Instant::now() + Duration::from_secs(5);
        while probes.status("down") != Reachability::Offline {
            assert!(Instant::now() < deadline, "the dead host was never given up on");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

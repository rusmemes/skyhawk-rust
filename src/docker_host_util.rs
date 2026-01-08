use std::net::UdpSocket;

pub fn get_docker_host() -> String {

    let hostname = hostname::get()
        .expect("Unable to get hostname")
        .to_string_lossy()
        .into_owned();

    if !hostname.is_empty() {
        return hostname;
    }

    let local_ip = UdpSocket::bind("0.0.0.0:0")
        .expect("Unable to bind UDP socket");
    local_ip.connect("8.8.8.8:80").expect("Unable to connect UDP socket");
    let local_ip = local_ip.local_addr().expect("Unable to get local IP").ip();

    let canonical = dns_lookup::lookup_addr(&local_ip)
        .unwrap_or_else(|_| "".to_string());

    if !canonical.is_empty() {
        canonical
    } else {
        panic!("unable to determine docker host");
    }
}

use std::{
    net::{IpAddr, Ipv4Addr},
    sync::OnceLock,
};

use local_ip_address::local_ip;

pub fn get_local_ip_str() -> String {
    get_local_ip().to_string()
}

pub fn get_local_ip() -> &'static IpAddr {
    static IP: OnceLock<IpAddr> = OnceLock::new();
    IP.get_or_init(|| local_ip().unwrap())
}

pub fn get_local_ip_u32() -> u32 {
    static IP: OnceLock<Ipv4Addr> = OnceLock::new();

    let ip = *IP.get_or_init(|| {
        let ip = local_ip().unwrap();
        match ip {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                panic!("only support ipv4");
            }
        }
    });
    u32::from(ip)
}

pub fn get_self_pid() -> u32 {
    static PID: OnceLock<u32> = OnceLock::new();
    *PID.get_or_init(|| std::process::id())
}

use anyhow::Error;
use anyhow::Result;
use get_if_addrs::get_if_addrs;
use std::cmp::Ordering;
use std::net::IpAddr;

// 获取当前服务实例的局域网ip
pub fn get_inst_default_ip() -> Result<String> {
    let ifaces = get_if_addrs()?;
    let mut addrs: Vec<IpAddr> = ifaces
        .iter()
        .map(|iface| iface.ip())
        .filter(|ip| {
            !ip.is_multicast()
                && match ip {
                    IpAddr::V4(ref v4) => !v4.is_link_local(),
                    IpAddr::V6(_) => true,
                }
        })
        .collect();
    addrs.sort_by(|a, b| {
        if a.is_loopback() {
            return Ordering::Greater;
        }
        if b.is_loopback() {
            return Ordering::Less;
        }
        if let IpAddr::V4(ref v4) = *a {
            if v4.is_private() && v4.octets()[0] == 172 {
                return Ordering::Greater;
            }
        }
        if let IpAddr::V4(ref v4) = *b {
            if v4.is_private() && v4.octets()[0] == 172 {
                return Ordering::Less;
            }
        }
        a.cmp(b)
    });
    if addrs.is_empty() {
        return Err(Error::msg("获取当前实例的ip地址失败"));
    }
    Ok(addrs[0].to_string())
}

#[cfg(test)]
mod tests {
    use log::debug;
    #[test]
    fn test_get_ip() {
        debug!("current ip is:{:?}", super::get_inst_default_ip());
    }
}

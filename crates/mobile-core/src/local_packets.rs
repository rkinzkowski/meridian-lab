//! Original local TCP packet reflection used by the entitled iOS Network Extension.
//! No packets are forwarded to a socket or external server.
use std::net::Ipv4Addr;

pub fn validate_pair(interface: Ipv4Addr, peer: Ipv4Addr) -> bool {
    let a = u32::from(interface);
    let b = u32::from(peer);
    interface.is_private()
        && peer.is_private()
        && a != b
        && (a & !3) == (b & !3)
        && [1, 2].contains(&(a & 3))
        && [1, 2].contains(&(b & 3))
}
fn sum(bytes: &[u8]) -> u32 {
    bytes
        .chunks(2)
        .map(|pair| ((pair[0] as u32) << 8) | pair.get(1).copied().unwrap_or(0) as u32)
        .sum()
}
fn checksum(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Accepts one nonfragmented TCP/IPv4 packet between the two configured local
/// addresses. Recomputes both checksums, including the TCP pseudo-header.
pub fn reflect(packet: &mut [u8], interface: Ipv4Addr, peer: Ipv4Addr) -> bool {
    if !validate_pair(interface, peer)
        || !(40..=65535).contains(&packet.len())
        || packet[0] != 0x45
        || packet[9] != 6
        || packet[8] == 0
    {
        return false;
    }
    let total = u16::from_be_bytes([packet[2], packet[3]]) as usize;
    let fragment = u16::from_be_bytes([packet[6], packet[7]]);
    if total != packet.len() || fragment & 0x3fff != 0 {
        return false;
    }
    let tcp_header = ((packet[32] >> 4) as usize) * 4;
    if tcp_header < 20 || tcp_header + 20 > total {
        return false;
    }
    let source = <[u8; 4]>::try_from(&packet[12..16]).unwrap();
    let destination = <[u8; 4]>::try_from(&packet[16..20]).unwrap();
    let pair = (interface.octets(), peer.octets());
    if (source, destination) != pair && (source, destination) != (pair.1, pair.0) {
        return false;
    }
    packet[12..16].copy_from_slice(&destination);
    packet[16..20].copy_from_slice(&source);
    packet[10..12].fill(0);
    let ip_checksum = checksum(sum(&packet[..20]));
    packet[10..12].copy_from_slice(&ip_checksum.to_be_bytes());
    packet[36..38].fill(0);
    let tcp_checksum =
        checksum(sum(&packet[12..20]) + 6 + (total - 20) as u32 + sum(&packet[20..]));
    packet[36..38].copy_from_slice(&tcp_checksum.to_be_bytes());
    true
}

/// # Safety
/// `bytes` must be uniquely writable for `length` bytes. Address integers use
/// network numeric order (10.0.0.1 is 0x0a000001), independent of host endian.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn meridian_reflect_local_tcp(
    bytes: *mut u8,
    length: usize,
    interface: u32,
    peer: u32,
) -> bool {
    if bytes.is_null() || !(40..=65535).contains(&length) {
        return false;
    }
    let packet = unsafe { std::slice::from_raw_parts_mut(bytes, length) };
    reflect(packet, interface.into(), peer.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pair() -> (Ipv4Addr, Ipv4Addr) {
        (Ipv4Addr::new(10, 83, 0, 2), Ipv4Addr::new(10, 83, 0, 1))
    }
    fn packet() -> Vec<u8> {
        let mut packet = vec![0u8; 43];
        packet[0] = 0x45;
        packet[2..4].copy_from_slice(&43u16.to_be_bytes());
        packet[8] = 64;
        packet[9] = 6;
        packet[12..16].copy_from_slice(&pair().0.octets());
        packet[16..20].copy_from_slice(&pair().1.octets());
        packet[20..22].copy_from_slice(&45000u16.to_be_bytes());
        packet[22..24].copy_from_slice(&62078u16.to_be_bytes());
        packet[32] = 0x50;
        packet[40..].copy_from_slice(b"abc");
        packet
    }
    #[test]
    fn reflector_swaps_only_addresses_and_checksums() {
        let mut p = packet();
        let before = p.clone();
        let (a, b) = pair();
        assert!(reflect(&mut p, a, b));
        assert_eq!(&p[12..16], &b.octets());
        assert_eq!(&p[16..20], &a.octets());
        assert_eq!(&p[20..36], &before[20..36]);
        assert_eq!(&p[38..], &before[38..]);
        assert_eq!(checksum(sum(&p[..20])), 0);
        assert_eq!(checksum(sum(&p[12..20]) + 6 + 23 + sum(&p[20..])), 0);
        assert!(reflect(&mut p, a, b));
        assert_eq!(&p[12..20], &before[12..20]);
    }
    #[test]
    fn reflector_rejects_external_malformed_fragmented_and_non_tcp_packets() {
        let (a, b) = pair();
        for index in [0, 2, 6, 9, 12, 16, 32] {
            let mut p = packet();
            p[index] = 0xff;
            assert!(!reflect(&mut p, a, b));
        }
        for length in 0..40 {
            assert!(!reflect(&mut vec![0; length], a, b));
        }
        assert!(!validate_pair(
            Ipv4Addr::new(8, 8, 8, 2),
            Ipv4Addr::new(8, 8, 8, 1)
        ));
        assert!(!validate_pair(a, a));
        assert!(!validate_pair(a, Ipv4Addr::new(10, 84, 0, 1)));
    }
}

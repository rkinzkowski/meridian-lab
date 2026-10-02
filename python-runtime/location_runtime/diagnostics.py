"""Allowlisted diagnostics: never include identities, pairing records or coordinates."""

import platform
import socket


def windows_ipv6() -> dict:
    if platform.system() != "Windows":
        return {"registryState": "not-applicable", "socketAvailable": socket.has_ipv6}
    import winreg

    try:
        with winreg.OpenKey(
            winreg.HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters"
        ) as key:
            value, _kind = winreg.QueryValueEx(key, "DisabledComponents")
    except FileNotFoundError:
        return {"registryState": "absent", "socketAvailable": socket.has_ipv6}
    except OSError:
        return {"registryState": "unreadable", "socketAvailable": socket.has_ipv6}
    return {
        "registryState": "present",
        "disabledComponents": value,
        "preferIpv4": bool(value & 0x20),
        "socketAvailable": socket.has_ipv6,
    }


def system_snapshot() -> dict:
    result = {
        "platform": platform.system(),
        "architecture": platform.machine(),
        "ipv6": windows_ipv6(),
        "transportPolicy": "userspace-default",
        "firewallModified": False,
    }
    try:
        import ifaddr

        result["adapters"] = [
            {
                "name": a.nice_name,
                "ipv4Count": sum(i.is_IPv4 for i in a.ips),
                "ipv6Count": sum(i.is_IPv6 for i in a.ips),
            }
            for a in ifaddr.get_adapters()
        ]
    except (ImportError, OSError):
        result["adapters"] = "unavailable"
    return result

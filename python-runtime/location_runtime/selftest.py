"""Offline bundle check. No discovery, pairing, or phone connection is performed."""

import asyncio
import json
import os
import ssl
import sys


def _psk_exchange(client_key: bytes, server_key: bytes):
    """Exercise the upstream TLS 1.2 PSK settings using memory buffers only."""
    client_context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client_context.check_hostname = False
    # PSK authenticates this connection; no certificate is exchanged. These
    # settings match pymobiledevice3's native-PSK TCP tunnel, not web requests.
    client_context.verify_mode = ssl.CERT_NONE
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    for context in (client_context, server_context):
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.maximum_version = ssl.TLSVersion.TLSv1_2
        context.set_ciphers("PSK")
    client_context.set_psk_client_callback(lambda hint: (None, client_key))
    server_context.set_psk_server_callback(lambda identity: server_key)
    client_in, client_out = ssl.MemoryBIO(), ssl.MemoryBIO()
    server_in, server_out = ssl.MemoryBIO(), ssl.MemoryBIO()
    client = client_context.wrap_bio(client_in, client_out, server_side=False)
    server = server_context.wrap_bio(server_in, server_out, server_side=True)
    completed = [False, False]
    for _ in range(32):
        for index, endpoint in enumerate((client, server)):
            if not completed[index]:
                try:
                    endpoint.do_handshake()
                    completed[index] = True
                except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                    pass
        for outgoing, incoming in ((client_out, server_in), (server_out, client_in)):
            if outgoing.pending:
                incoming.write(outgoing.read())
        if all(completed):
            payload = b"Meridian offline TLS-PSK check"
            client.write(payload)
            server_in.write(client_out.read())
            if server.read(len(payload)) != payload:
                raise RuntimeError("TLS-PSK payload did not round-trip")
            return client.version(), client.cipher()[0]
    raise RuntimeError("In-memory TLS-PSK handshake did not complete")


def check_tls_psk():
    """Verify native PSK negotiation and rejection of an incorrect synthetic key."""
    if not getattr(ssl, "HAS_PSK", False):
        raise RuntimeError("This runtime requires Python 3.13+ with native TLS-PSK support")
    key = os.urandom(32)
    protocol, cipher = _psk_exchange(key, key)
    if protocol != "TLSv1.2" or "PSK" not in cipher:
        raise RuntimeError("Unexpected TLS-PSK negotiation")
    incorrect_key = bytes([key[0] ^ 1]) + key[1:]
    try:
        _psk_exchange(key, incorrect_key)
    except ssl.SSLError:
        pass
    else:
        raise RuntimeError("TLS-PSK accepted an incorrect key")
    return {"protocol": protocol, "cipher": cipher, "incorrectKeyRejected": True}


async def check():
    report = {
        "phoneAccessed": False,
        "pythonVersion": sys.version.split()[0],
        "opensslVersion": ssl.OPENSSL_VERSION,
        "wirelessTlsPskAvailable": False,
        "steps": [],
    }
    tunnel = None
    try:
        from pymobiledevice3.remote.userspace_tunnel import UserspaceTun
        from pymobiledevice3.services.dvt.instruments.location_simulation import LocationSimulation

        assert LocationSimulation
        report["steps"].append("developer imports available")
        report["tlsPsk"] = check_tls_psk()
        report["wirelessTlsPskAvailable"] = True
        report["steps"].append("in-memory TLS-PSK handshake and incorrect-key rejection passed")
        tunnel = UserspaceTun()
        tunnel.addr = "fd00:1234::1"
        async with asyncio.timeout(8):
            await tunnel.up()
        report["steps"].append("isolated userspace stack started")
        await tunnel.close()
        tunnel = None
        report["steps"].append("isolated userspace stack closed")
        report["result"] = "PASS"
        report["scope"] = "USB and Wi-Fi runtime prerequisites; no physical session tested"
    except Exception as exc:
        report["result"] = "FAIL"
        report["failureClass"] = type(exc).__name__
        report["detail"] = str(exc)[:1000]
        # This self-test uses only synthetic data; module import names are safe.
        if isinstance(exc, ImportError):
            report["missingModule"] = exc.name
    finally:
        if tunnel:
            await tunnel.close()
    print(json.dumps(report), flush=True)
    return 0 if report["result"] == "PASS" else 1

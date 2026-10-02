import ssl

import pytest

from location_runtime.selftest import check_tls_psk


@pytest.mark.skipif(not getattr(ssl, "HAS_PSK", False), reason="native PSK requires Python 3.13+")
def test_psk_negotiates_and_rejects_wrong_key():
    report = check_tls_psk()
    assert report["protocol"] == "TLSv1.2"
    assert "PSK" in report["cipher"]
    assert report["incorrectKeyRejected"] is True


def test_psk_unavailable_fails_the_prerequisite_check(monkeypatch):
    monkeypatch.setattr(ssl, "HAS_PSK", False, raising=False)
    with pytest.raises(RuntimeError, match="native TLS-PSK"):
        check_tls_psk()

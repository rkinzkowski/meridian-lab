import json

import pytest

from location_runtime.errors import RuntimeFault, classify
from location_runtime.protocol import parse_request, usable_address, validate_point


@pytest.mark.parametrize(
    "latitude,longitude",
    [(float("nan"), 0), (0, float("inf")), (True, 0), ("1", 0), (91, 0), (0, -181), (None, 1)],
)
def test_invalid_coordinates(latitude, longitude):
    with pytest.raises(RuntimeFault):
        validate_point(latitude, longitude)


@pytest.mark.parametrize("latitude,longitude", [(0, 0), (90, 180), (-90, -180), (40.7128, -74.006)])
def test_coordinate_boundaries(latitude, longitude):
    assert validate_point(latitude, longitude) == (latitude, longitude)


@pytest.mark.parametrize(
    "payload",
    [
        {"requestId": "a", "method": "shell", "params": {}},
        {"requestId": "a", "method": "location.clear", "params": {"path": "../x"}},
        {"requestId": "a\nREADY", "method": "runtime.info", "params": {}},
        {
            "requestId": "a",
            "method": "session.open",
            "params": {
                "udid": "A" * 24,
                "transport": "rsd",
                "rsdAddress": "fe80::1",
                "rsdPort": 10,
            },
        },
    ],
)
def test_reject_unknown_fields_and_invalid_endpoints(payload):
    with pytest.raises(RuntimeFault):
        parse_request(json.dumps(payload).encode())


def test_duplicate_json_fields_rejected():
    with pytest.raises(RuntimeFault):
        parse_request(b'{"requestId":"a","requestId":"b","method":"runtime.info","params":{}}')


def test_wireless_addresses():
    assert usable_address("192.168.2.4")
    assert usable_address("fe80::1%12")
    for address in (
        "0.0.0.0",
        "169.254.2.3",
        "224.0.0.1",
        "fe80::1",
        "::",
        "127.0.0.1",
        "255.255.255.255",
        "bad",
    ):
        assert not usable_address(address)


def test_error_contents_do_not_escape():
    secret = "private_key=secret token=secret UDID-123 coordinates=12.345"
    assert secret not in json.dumps(classify(ValueError(secret)).wire())


def test_missing_service_is_not_ddi():
    from pymobiledevice3.exceptions import InvalidServiceError

    assert (
        classify(InvalidServiceError("anything", "TEST", "27.0"), "dvt").code
        == "DEVELOPER_SERVICES_UNAVAILABLE"
    )

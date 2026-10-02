"""Strict JSONL validation. No file paths or shell commands are accepted."""

import json
import math
import re
from dataclasses import dataclass
from ipaddress import ip_address
from typing import Any

from .errors import fault

MAX_LINE = 1024 * 1024
METHOD_FIELDS = {
    "runtime.info": (set(), set()),
    "runtime.shutdown": (set(), set()),
    "device.list": (set(), set()),
    "device.trust": ({"udid"}, {"udid"}),
    "device.prepare": ({"udid"}, {"udid"}),
    "session.open": ({"udid", "transport"}, {"udid", "transport", "rsdAddress", "rsdPort"}),
    "session.close": (set(), set()),
    "session.health": (set(), set()),
    "location.set": ({"latitude", "longitude"}, {"latitude", "longitude"}),
    "location.clear": (set(), set()),
    "pairing.prepare": ({"udid"}, {"udid"}),
    "diagnostics.get": (set(), set()),
    "wireless.discover": (set(), set()),
    "operation.cancel": ({"targetRequestId"}, {"targetRequestId"}),
}


def validate_udid(value: Any) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9-]{16,64}", value):
        raise fault("INVALID_REQUEST")
    return value


def validate_point(latitude: Any, longitude: Any) -> tuple[float, float]:
    for value, limit in [(latitude, 90), (longitude, 180)]:
        if type(value) not in (float, int) or not math.isfinite(value) or abs(value) > limit:
            raise fault("INVALID_REQUEST")
    return float(latitude), float(longitude)


def usable_address(text: str, *, allow_loopback=False) -> bool:
    try:
        address = ip_address(text)
        if address.is_loopback:
            return allow_loopback
        if address.is_multicast or address.is_unspecified or address.is_reserved:
            return False
        if address.is_loopback and not allow_loopback:
            return False
        if address.is_link_local:
            return address.version == 6 and bool(getattr(address, "scope_id", None))
        return True
    except ValueError:
        return False


@dataclass(frozen=True)
class Request:
    request_id: str
    method: str
    params: dict[str, Any]


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise fault("INVALID_REQUEST")
        result[key] = value
    return result


def parse_request(line: bytes) -> Request:
    if len(line) > MAX_LINE:
        raise fault("INVALID_REQUEST")
    try:
        data = json.loads(line, object_pairs_hook=_unique_object)
    except (ValueError, UnicodeDecodeError, RecursionError) as exc:
        raise fault("INVALID_REQUEST") from exc
    if not isinstance(data, dict) or set(data) != {"requestId", "method", "params"}:
        raise fault("INVALID_REQUEST")
    request_id, method, params = data["requestId"], data["method"], data["params"]
    if not isinstance(request_id, str) or not re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", request_id):
        raise fault("INVALID_REQUEST")
    if not isinstance(method, str) or method not in METHOD_FIELDS or not isinstance(params, dict):
        raise fault("INVALID_REQUEST")
    required, allowed = METHOD_FIELDS[method]
    if not required <= set(params) <= allowed:
        raise fault("INVALID_REQUEST")
    if "udid" in params:
        validate_udid(params["udid"])
    if method == "location.set":
        validate_point(params["latitude"], params["longitude"])
    if method == "operation.cancel":
        target = params["targetRequestId"]
        if not isinstance(target, str) or not re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", target):
            raise fault("INVALID_REQUEST")
    if method == "session.open":
        if params["transport"] not in ("usb", "wifi", "rsd"):
            raise fault("INVALID_REQUEST")
        if params["transport"] == "rsd":
            address, port = params.get("rsdAddress"), params.get("rsdPort")
            if not isinstance(address, str) or not usable_address(address, allow_loopback=True):
                raise fault("INVALID_REQUEST")
            if type(port) is not int or not 1 <= port <= 65535:
                raise fault("INVALID_REQUEST")
        elif "rsdAddress" in params or "rsdPort" in params:
            raise fault("INVALID_REQUEST")
    return Request(request_id, method, params)

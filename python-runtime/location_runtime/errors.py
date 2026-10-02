"""Stable, deliberately non-sensitive errors across the process boundary."""

from dataclasses import asdict, dataclass


@dataclass
class RuntimeFault(Exception):
    code: str
    message: str
    recovery: str
    retryable: bool = False

    def wire(self):
        return asdict(self)


def fault(code: str) -> RuntimeFault:
    message, recovery, retryable = ERRORS.get(code, ERRORS["UNKNOWN"])
    return RuntimeFault(code, message, recovery, retryable)


ERRORS = {
    "INVALID_REQUEST": (
        "This request is invalid.",
        "Check the selected device and input values.",
        False,
    ),
    "DEVICE_NOT_FOUND": (
        "The selected iPhone is not connected.",
        "Connect and unlock that iPhone over USB.",
        True,
    ),
    "DEVICE_DISCONNECTED": ("The iPhone disconnected.", "Reconnect the same iPhone.", True),
    "DEVICE_IDENTITY_MISMATCH": (
        "The connection belongs to a different device.",
        "Disconnect and select the intended iPhone again.",
        False,
    ),
    "DEVICE_LOCKED": ("The iPhone is locked.", "Unlock the iPhone and try again.", True),
    "TRUST_REQUIRED": (
        "This computer is not trusted.",
        "Choose Trust Computer, then accept Trust on the unlocked iPhone.",
        False,
    ),
    "PAIRING_REJECTED": (
        "The iPhone declined pairing.",
        "Start pairing again when you are ready to approve it.",
        False,
    ),
    "STALE_PAIRING_RECORD": (
        "The saved trust relationship was rejected.",
        "Choose Trust Computer to create a fresh legitimate pairing.",
        False,
    ),
    "USBMUX_UNAVAILABLE": (
        "Apple device communication is unavailable.",
        "Install or finish setting up Apple Devices, then connect the iPhone.",
        True,
    ),
    "DEVELOPER_MODE_DISABLED": (
        "Developer Mode is disabled.",
        "Enable Developer Mode in Settings, restart the iPhone, and confirm it.",
        False,
    ),
    "DEVELOPER_SUPPORT_MISSING": (
        "Developer support is not mounted.",
        "Choose Prepare Developer Support, then connect again.",
        False,
    ),
    "DEVELOPER_SUPPORT_MOUNT_FAILED": (
        "Developer support could not be prepared.",
        "Check your internet connection and the iOS build, then retry preparation.",
        True,
    ),
    "DEVELOPER_SERVICES_UNAVAILABLE": (
        "The iPhone is not publishing its developer services.",
        "Confirm Developer Mode on the iPhone; prepare developer support if needed.",
        False,
    ),
    "ADMIN_REQUIRED": (
        "This transport requires system authorization.",
        "Use the supported USB userspace transport or configure a scoped tunnel helper.",
        False,
    ),
    "LEGACY_TUNNEL_REQUIRED": (
        "This iOS version needs a different developer tunnel.",
        "For iOS 17.0–17.3, use an explicitly configured RSD tunnel; this automatic path needs iOS 17.4 or newer.",
        False,
    ),
    "WIRELESS_NOT_PREPARED": (
        "Wi-Fi development has not been prepared.",
        "Connect by USB and choose Prepare Wi-Fi.",
        False,
    ),
    "WIRELESS_DEVICE_NOT_ADVERTISING": (
        "The prepared iPhone was not found on Wi-Fi.",
        "Keep both devices on the same private network and wake the iPhone.",
        True,
    ),
    "REMOTE_PAIR_VERIFY_REJECTED": (
        "The iPhone rejected its Wi-Fi pairing.",
        "Reconnect by USB and prepare Wi-Fi again.",
        False,
    ),
    "WIRELESS_TUNNEL_UNREACHABLE": (
        "The iPhone's Wi-Fi tunnel could not be reached.",
        "Check the private network and local firewall policy, then retry.",
        True,
    ),
    "RSD_UNAVAILABLE": (
        "The developer discovery connection is unavailable.",
        "Reconnect the selected iPhone and rebuild the tunnel.",
        True,
    ),
    "DVT_FAILED": (
        "The developer service did not open.",
        "Reconnect and confirm Developer Mode and developer support.",
        True,
    ),
    "CONNECTION_REFUSED": (
        "The iPhone refused the connection.",
        "Unlock it and reconnect the developer session.",
        True,
    ),
    "CONNECTION_TIMEOUT": (
        "The device operation timed out.",
        "Unlock the iPhone and reconnect; check USB or Wi-Fi connectivity.",
        True,
    ),
    "CONNECTION_DROPPED": (
        "The developer connection was lost.",
        "Reconnect the same iPhone before resuming.",
        True,
    ),
    "LOCATION_SESSION_FAILED": (
        "There is no healthy location session.",
        "Connect the selected iPhone first.",
        True,
    ),
    "SESSION_ALREADY_OPEN": (
        "A device session is already open.",
        "Restore and disconnect before changing devices or transports.",
        False,
    ),
    "RESTORE_FAILED": (
        "Restoring real location could not be confirmed.",
        "Reconnect and choose Restore Real Location. If that fails, restart the iPhone.",
        False,
    ),
    "CLEANUP_FAILED": (
        "Some device resources did not close cleanly.",
        "Restart the runtime before opening another session.",
        True,
    ),
    "DEPENDENCY_MISMATCH": (
        "The bundled device runtime version is unexpected.",
        "Rebuild or reinstall the complete Meridian Lab runtime.",
        False,
    ),
    "RUNTIME_DEPENDENCY_MISSING": (
        "A required developer runtime component could not be loaded.",
        "Rebuild or reinstall the complete runtime, then run its offline self-test.",
        False,
    ),
    "WIRELESS_TLS_UNAVAILABLE": (
        "This runtime cannot open the authenticated Wi-Fi tunnel.",
        "Use USB, or rebuild the runtime with Python 3.13 or newer for native TLS-PSK support.",
        False,
    ),
    "BUSY": (
        "Too many device operations are pending.",
        "Wait for the current operation to finish.",
        True,
    ),
    "CANCELLED": ("The operation was cancelled.", "Start it again when ready.", False),
    "UNKNOWN": (
        "The device operation failed.",
        "Check diagnostics and reconnect the iPhone.",
        False,
    ),
}


def classify(exc: BaseException, stage: str = "") -> RuntimeFault:
    if isinstance(exc, RuntimeFault):
        return exc
    # Inspect class hierarchy without importing optional third-party exception groups.
    # Only class names, never exception contents, leave this boundary.
    names = {cls.__name__ for cls in type(exc).__mro__}
    groups = [
        (
            {"ImportError", "ModuleNotFoundError", "PyInstallerImportError"},
            "RUNTIME_DEPENDENCY_MISSING",
        ),
        ({"DeveloperModeIsNotEnabledError", "DeveloperModeError"}, "DEVELOPER_MODE_DISABLED"),
        ({"NotMountedError", "DeveloperDiskImageNotFoundError"}, "DEVELOPER_SUPPORT_MISSING"),
        ({"PasscodeRequiredError", "PasswordRequiredError"}, "DEVICE_LOCKED"),
        ({"UserDeniedPairingError"}, "PAIRING_REJECTED"),
        ({"InvalidHostIDError", "InvalidConnectionError"}, "STALE_PAIRING_RECORD"),
        ({"NotPairedError", "PairingDialogResponsePendingError"}, "TRUST_REQUIRED"),
        ({"NoDeviceConnectedError", "DeviceNotFoundError"}, "DEVICE_NOT_FOUND"),
        ({"ConnectionRefusedError", "ConnectionFailedError"}, "CONNECTION_REFUSED"),
        ({"TimeoutError"}, "CONNECTION_TIMEOUT"),
        (
            {
                "ConnectionTerminatedError",
                "ConnectionResetError",
                "ConnectionAbortedError",
                "BrokenPipeError",
                "IncompleteReadError",
            },
            "CONNECTION_DROPPED",
        ),
        ({"PermissionError"}, "ADMIN_REQUIRED"),
        ({"UserspaceTunnelUnavailableError"}, "LEGACY_TUNNEL_REQUIRED"),
        (
            {
                "DTXNsError",
                "DTXProtocolError",
                "DTXNSCodingError",
                "DvtException",
                "StartServiceError",
            },
            "DVT_FAILED",
        ),
    ]
    if "InvalidServiceError" in names:
        return fault("DEVELOPER_SERVICES_UNAVAILABLE" if stage == "dvt" else "RSD_UNAVAILABLE")
    for classes, code in groups:
        if names & classes:
            return fault(code)
    if "ConnectionError" in names:
        return fault("CONNECTION_DROPPED")
    return fault("UNKNOWN")

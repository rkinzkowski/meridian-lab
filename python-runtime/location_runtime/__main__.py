import asyncio
import logging
import sys

from .driver import PmdDriver
from .server import Server


def main():
    # Third-party debug logs may contain pair records and identifiers. Export
    # only our allowlisted diagnostics. Raw third-party logging is disabled.
    logging.disable(logging.CRITICAL)
    try:
        if sys.argv[1:] == ["--self-test"]:
            from .selftest import check

            return asyncio.run(check())
        if len(sys.argv) != 1:
            sys.stderr.write(
                "Use --self-test or start the private JSONL runtime without arguments.\n"
            )
            return 2
        asyncio.run(Server(PmdDriver()).run())
        return 0
    except KeyboardInterrupt:
        return 130
    except Exception:
        sys.stderr.write("Meridian runtime terminated; restart the device session.\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Opt-in Browser preflight; run --help for the separate campaign start gate."""

from __future__ import annotations

import argparse
import json
import sys

from browser_preflight.campaign import freeze, gate, launch
from browser_preflight.evidence import PreflightError, read_json
from browser_preflight.playwright_probe import probe
from browser_preflight.session import finish, prepare


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for name in ("prepare", "probe"):
        command = sub.add_parser(name)
        command.add_argument("--request", required=True)
        command.add_argument("--ledger", required=True)
        command.add_argument("--reason", default="")
    command = sub.add_parser("record")
    command.add_argument("--ticket", required=True)
    command.add_argument("--observation", required=True)
    command = sub.add_parser("freeze")
    command.add_argument("--request", required=True)
    command.add_argument("--report", required=True)
    command.add_argument(
        "--campaign", required=True, help="New directory; must not exist"
    )
    command.add_argument("--campaign-id", required=True)
    for name in ("gate", "launch"):
        command = sub.add_parser(name)
        command.add_argument("--request", required=True)
        command.add_argument("--manifest", required=True)
        if name == "launch":
            command.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    try:
        if args.action in ("prepare", "probe"):
            request = read_json(args.request)
            if (
                args.action == "probe"
                and request.get("method") != "isolated-playwright"
            ):
                raise PreflightError(
                    "Use prepare + the supported Browser tool for plugin probes"
                )
            result = prepare(args.ledger, request, args.reason)
            if result["decision"] == "probe" and not result["operation_allowed"]:
                result = {"report": str(finish(result["ticket"], {}))}
            elif result["decision"] == "probe" and args.action == "probe":
                result = {
                    "report": str(finish(result["ticket"], probe(result["ticket"])))
                }
        elif args.action == "record":
            result = {"report": str(finish(args.ticket, read_json(args.observation)))}
        elif args.action == "freeze":
            result = {
                "manifest": str(
                    freeze(
                        args.campaign,
                        args.campaign_id,
                        args.report,
                        read_json(args.request),
                    )
                )
            }
        elif args.action == "gate":
            result = gate(args.manifest, read_json(args.request))
        else:
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            return launch(args.manifest, read_json(args.request), command)
        print(json.dumps(result, indent=2))
        if "report" in result and read_json(result["report"])["status"] != "passed":
            return 2
        return 0
    except (
        PreflightError,
        OSError,
        ValueError,
        KeyError,
        TypeError,
        IndexError,
    ) as error:
        print(json.dumps({"status": "blocked", "error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

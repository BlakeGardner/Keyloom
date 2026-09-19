#!/usr/bin/env python3
"""Drives the Open Build Service for the release workflows.

Everything but `trigger` uses the public, unauthenticated API. `trigger`
needs a service token in OBS_TOKEN; such a token can only run a package's
source services, which is all the workflows need. See packaging/README.md.

Subcommands:
  trigger PROJECT PACKAGE
      Run the package's source services (its _service file), which fetch
      the source package from the GitHub release.
  wait-sources PROJECT PACKAGE FILE
      Wait until the services have produced FILE (for example
      keyloom_0.1.0-1.dsc). Fails when the services report an error or
      when they end with a different version than expected.
  wait-builds PROJECT PACKAGE
      Wait until every repository has finished building the package and
      report the outcome of each. Does not fail on failed builds, so the
      successful ones can still be fetched; run `check` afterwards.
  check PROJECT PACKAGE
      Fail if any build of the package did not succeed.
  fetch PROJECT PACKAGE DEST
      Download the .deb and .rpm files of every successful build into
      DEST, each renamed to say which distribution it is for
      (keyloom_0.1.0-1_amd64.deb from xUbuntu_24.04 becomes
      keyloom_0.1.0-1_amd64_ubuntu-24.04.deb, and
      keyloom-0.1.0-7.1.x86_64.rpm from Fedora_43 becomes
      keyloom-0.1.0-7.1.x86_64_fedora-43.rpm). Source and debug-symbol
      packages are skipped.
"""

import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

API = os.environ.get("OBS_API", "https://api.opensuse.org")
# Show progress as it happens in a CI log rather than at the end.
sys.stdout.reconfigure(line_buffering=True)
WAITING = {"blocked", "scheduled", "dispatching", "building", "signing", "finished"}
SKIPPED = {"excluded", "disabled"}


def die(message):
    print(message, file=sys.stderr)
    sys.exit(1)


class ServiceInProgress(Exception):
    pass


def request(path, method="GET", token=None):
    headers = {"User-Agent": "keyloom-release"}
    if token:
        headers["Authorization"] = f"Token {token}"
    req = urllib.request.Request(API + path, method=method, headers=headers)
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=300) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            body = error.read().decode(errors="replace")
            if error.code == 400 and "service in progress" in body:
                raise ServiceInProgress() from error
            # The API answers 502/504 now and then under load.
            if error.code not in (502, 503, 504) or attempt == 4:
                error.body = body
                raise
        except (urllib.error.URLError, TimeoutError):
            if attempt == 4:
                raise
        time.sleep(15)


def fetch_xml(path):
    return ET.fromstring(request(path))


def quoted(*parts):
    return "/".join(urllib.parse.quote(part, safe="") for part in parts)


def trigger(project, package):
    token = os.environ.get("OBS_TOKEN")
    if not token:
        die("OBS_TOKEN is not set")
    query = urllib.parse.urlencode({"project": project, "package": package})
    try:
        body = request(f"/trigger/runservice?{query}", method="POST", token=token)
    except urllib.error.HTTPError as error:
        die(f"Could not trigger the services of {project}/{package}: HTTP {error.code}\n{error.body}")
    print(body.decode(errors="replace").strip())


def source_state(project, package):
    """Returns (service state or None, error text or None, file names without the _service: prefix)."""
    try:
        info = fetch_xml(f"/public/source/{quoted(project, package)}?view=info")
        service = info.find("serviceinfo")
        state = service.get("code") if service is not None else None
        error = service.findtext("error") if service is not None else None
        # Only the expanded listing includes the files the services generated,
        # and OBS refuses to expand while a service is still running.
        listing = fetch_xml(f"/public/source/{quoted(project, package)}?expand=1")
    except ServiceInProgress:
        return "running", None, set()
    names = set()
    for entry in listing.findall("entry"):
        name = entry.get("name")
        if name.startswith("_service:"):
            name = name.split(":", 2)[2]
        names.add(name)
    return state, error, names


def wait_sources(project, package, expected):
    timeout = int(os.environ.get("OBS_SOURCES_TIMEOUT", str(20 * 60)))
    print(f"Waiting for the services of {project}/{package} to produce {expected}")
    deadline = time.monotonic() + timeout
    last = None
    while True:
        state, error, names = source_state(project, package)
        if state == "failed" or "_service_error" in names:
            detail = error or request(f"/public/source/{quoted(project, package, '_service_error')}").decode(errors="replace")
            die(f"The source services failed:\n{detail}")
        if state != "running" and expected in names:
            print("Sources are in place")
            return
        others = sorted(n for n in names if n.endswith(".dsc") and n != expected)
        status = (state, tuple(others))
        if status != last:
            print(f"  service state: {state or 'unknown'}; source packages present: {', '.join(others) or 'none'}")
            last = status
        if time.monotonic() > deadline:
            hint = ""
            if others:
                hint = (f"\nOBS holds {', '.join(others)} instead. Its _service file fetches the bundle of GitHub's "
                        "latest release; check that the release this run is for is the latest one.")
            die(f"Timed out waiting for {expected}{hint}")
        time.sleep(15)


def results(project, package):
    root = fetch_xml(f"/public/build/{quoted(project)}/_result?{urllib.parse.urlencode({'package': package})}")
    rows = []
    for result in root.findall("result"):
        status = result.find("status")
        code = status.get("code") if status is not None else result.get("code")
        details = status.findtext("details") if status is not None else None
        rows.append({
            "repository": result.get("repository"),
            "arch": result.get("arch"),
            "code": code,
            "details": details or "",
            "dirty": result.get("dirty") == "true",
            "repo_code": result.get("code"),
        })
    return rows


def wait_builds(project, package):
    settle = int(os.environ.get("OBS_SETTLE_SECONDS", "90"))
    print(f"Giving the scheduler {settle}s to notice the new sources")
    time.sleep(settle)
    last = None
    while True:
        rows = results(project, package)
        pending = [r for r in rows if r["dirty"] or r["code"] in WAITING or r["repo_code"] in WAITING]
        summary = ", ".join(f"{r['repository']}/{r['arch']}={r['code']}" for r in rows)
        if summary != last:
            print(f"  {summary}")
            last = summary
        if not pending:
            break
        time.sleep(60)
    print("All builds finished")
    check(project, package, fatal=False)


def check(project, package, fatal=True):
    rows = results(project, package)
    failed = [r for r in rows if r["code"] not in {"succeeded"} | SKIPPED]
    for r in failed:
        print(f"::warning::{package} on {r['repository']}/{r['arch']}: {r['code']} {r['details']}".rstrip())
    if failed and fatal:
        die(f"Some builds did not succeed; see https://build.opensuse.org/package/show/{project}/{package}")
    if not failed:
        print("All builds succeeded")


def distribution_name(repository):
    # Debian_13 -> debian-13, xUbuntu_24.04 -> ubuntu-24.04, Fedora_Rawhide -> fedora-rawhide
    if repository.startswith("xUbuntu"):
        repository = repository[1:]
    return repository.replace("_", "-").lower()


def is_package(name):
    """Whether a build result file is a binary package worth attaching to the release."""
    if name.endswith(".deb"):
        return "-dbgsym_" not in name
    if name.endswith(".rpm"):
        return not name.endswith(".src.rpm") and "-debuginfo-" not in name and "-debugsource-" not in name
    return False


def fetch(project, package, dest):
    os.makedirs(dest, exist_ok=True)
    for r in results(project, package):
        if r["code"] != "succeeded":
            continue
        base = f"/public/build/{quoted(project, r['repository'], r['arch'], package)}"
        for binary in fetch_xml(base).findall("binary"):
            name = binary.get("filename")
            if not is_package(name):
                continue
            stem, extension = os.path.splitext(name)
            target = os.path.join(dest, f"{stem}_{distribution_name(r['repository'])}{extension}")
            with open(target, "wb") as out:
                out.write(request(f"{base}/{urllib.parse.quote(name)}"))
            print(f"  {os.path.basename(target)}")


def main(argv):
    commands = {"trigger": (trigger, 2), "wait-sources": (wait_sources, 3), "wait-builds": (wait_builds, 2),
                "check": (check, 2), "fetch": (fetch, 3)}
    if len(argv) < 2 or argv[1] not in commands:
        die(__doc__)
    function, arity = commands[argv[1]]
    if len(argv) - 2 != arity:
        die(__doc__)
    function(*argv[2:])


if __name__ == "__main__":
    main(sys.argv)

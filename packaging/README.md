# Packaging

Keyloom's Debian and Ubuntu packages are built on the
[Open Build Service](https://build.opensuse.org/) (OBS). Publishing a GitHub
release starts the pipeline; nothing is built on GitHub's runners except
the source package.

```text
GitHub release published
  └─ .github/workflows/release.yml
       ├─ scripts/build-deb-source.sh    sources + vendored crates → .dsc
       ├─ gh release upload              keyloom-debian-source.tar on the release
       ├─ scripts/obs.py trigger         service token → OBS runs its _service:
       │    └─ OBS downloads the bundle from the release, unpacks it,
       │       and builds keyloom for every repository and architecture
       └─ scripts/obs.py wait + fetch    attach the .debs to the release
```

OBS never receives an account credential: the only secret GitHub holds is
a service token that can do nothing but run one package's source services,
and everything the workflow reads from OBS comes from its public API.

Users install from the OBS repository for their distribution (OBS publishes
an apt repository per distribution under
`https://download.opensuse.org/repositories/<project>/`) or download a
`.deb` from the GitHub release.

## Why a private Rust toolchain

libcosmic and its dependencies need Rust 1.93 or newer. Debian 13 ships
1.85 and Ubuntu 24.04 backports stop at 1.91, and OBS builds have no
network access, so the build service cannot use `rustup`. The
`keyloom-rust-toolchain` package (`packaging/rust-toolchain`) wraps
upstream's `rustc`, `rust-std`, and `cargo` release tarballs for x86_64 and
aarch64, verified against the checksums published on static.rust-lang.org,
and installs them under `/usr/lib/keyloom-rust-toolchain`. Every Keyloom
build uses it, on every distribution, so the packages are built the same
way everywhere. Its source package lives on the GitHub pre-release tagged
`rust-toolchain`, a fixed address the OBS package fetches from. Change
`packaging/rust-toolchain/version` to move to a newer Rust; pushing that
change to `main` replaces the bundle and re-triggers OBS
(`.github/workflows/rust-toolchain.yml`).

The Keyloom source package carries the crates it depends on under
`vendor/`, filtered to Linux targets, so OBS can build offline. The build
recipe is `packaging/debian`; `scripts/build-deb-source.sh` adds the
generated `debian/changelog`.

## One-time setup

### 1. Create the OBS project and packages

With an account on https://build.opensuse.org, either use the web
interface:

1. Open your home project, choose **Subprojects → Create subproject**, and
   name it `Keyloom` (the project becomes `home:<username>:Keyloom`). A
   subproject keeps Keyloom's repositories separate from anything else you
   build. Leave the SCM (git) URL field empty: a project "managed in SCM"
   takes its packages from that repository and offers no way to create
   them by hand. If it was set, remove the `<scmsync>` line on the
   project's **Meta** tab (under **Advanced**) and save.
2. On the project's **Repositories** tab, **Add from a Distribution** for
   Debian 13, Debian Testing, Debian Unstable, Ubuntu 24.04, Ubuntu 25.10,
   and Ubuntu 26.04. Edit each repository to add the `aarch64` architecture
   next to `x86_64`.
3. Create two packages in the project: `keyloom` and
   `keyloom-rust-toolchain` (**Create Package**, name only).
4. In each package, **Add file** and upload the matching file from
   `packaging/obs/` with the name `_service`:
   `keyloom._service` for `keyloom` and `keyloom-rust-toolchain._service`
   for `keyloom-rust-toolchain`. These tell OBS where on GitHub to fetch
   the sources from. The first service run fails until the workflows have
   published something there; that is expected.

Or do the same from a terminal with `osc` (`pipx install osc`;
`osc ls home:<username>` asks for and stores your credentials the first
time):

```sh
project="home:<username>:Keyloom"
sed "s/OBS_USERNAME/<username>/g" packaging/obs/project.xml | osc meta prj -F - "$project"
for package in keyloom keyloom-rust-toolchain; do
    osc meta pkg -F - "$project" "$package" <<XML
<package name="$package"><title>$package</title><description/></package>
XML
    osc checkout "$project" "$package"
    cp "packaging/obs/$package._service" "$project/$package/_service"
    (cd "$project/$package" && osc add _service && osc commit -m "Fetch sources from GitHub")
done
```

### 2. Create the service tokens

On build.opensuse.org open **Your Profile → Tokens → Create Token** and
create two tokens of type **service**, each limited to the project and
one package: one for `keyloom`, one for `keyloom-rust-toolchain`. A
service token can only run that package's `_service`; it cannot read or
change anything else. Then, in the GitHub repository under **Settings →
Secrets and variables → Actions**:

- Secret `OBS_TOKEN_KEYLOOM`: the token for `keyloom`.
- Secret `OBS_TOKEN_TOOLCHAIN`: the token for `keyloom-rust-toolchain`.
- Variable `OBS_PROJECT`: `home:<username>:Keyloom`.

Nothing else is needed: the workflows read build states and download the
finished packages through OBS's public API.

### 3. Publish the Rust toolchain package

Run the **Rust toolchain package** workflow from the **Actions** tab
(**Run workflow**). It downloads the pinned Rust release, builds the source
package, publishes it on a GitHub pre-release tagged `rust-toolchain`
(clearly marked as not being a Keyloom release), triggers the OBS package,
and waits for OBS to build it for every repository. The first Keyloom
build in a repository waits for this package to be built there.

### 4. Publish a release

Bump `version` in `Cargo.toml`, commit, and publish a GitHub release whose
tag is `v` followed by that version, for example `v0.1.0`. The **Release
packages** workflow refuses a tag that does not match `Cargo.toml`.

The workflow attaches `keyloom-debian-source.tar` to the release, triggers
the OBS package, waits for OBS to finish every build (up to five hours),
and attaches the `.deb` files to the release, named like
`keyloom_0.1.0-1_amd64_ubuntu-24.04.deb`. A distribution whose build
failed is reported and fails the run, but only after the packages of the
others are attached: OBS's base system for a rolling distribution such as
Debian Unstable breaks now and then through no fault of the package, and
that should not withhold the rest. Build logs live on OBS:
`https://build.opensuse.org/package/show/<project>/keyloom`. To retry the
OBS part for an existing release, run the workflow by hand with the tag as
input.

OBS fetches the bundle from GitHub's *latest* release, so two things
follow. A release marked as a pre-release (a version such as
`0.2.0-rc.1`) is skipped by the OBS steps and exists only on GitHub. And
re-running the workflow for an older release than the latest one fails at
the "wait for OBS to take the sources" step, because OBS fetched the
latest release instead.

## Building locally

Both source packages can be built on a developer machine without touching
OBS; the scripts need `dpkg-dev`, and the Keyloom one also
`cargo-vendor-filterer` (`cargo install --locked cargo-vendor-filterer`):

```sh
scripts/build-rust-toolchain-source.sh /tmp/keyloom-packages
scripts/build-deb-source.sh /tmp/keyloom-packages
```

To build the binary packages the way OBS does, unpack each `.dsc` with
`dpkg-source -x` in a clean chroot or container of the target release,
install its build dependencies (`apt-get build-dep ./`), and run
`dpkg-buildpackage -us -uc -b`. Build and install `keyloom-rust-toolchain`
first; the `keyloom` package depends on it.

## Not covered yet

- Only Debian-format packages are produced. RPM, Flatpak, and other
  formats are not set up.
- Packages are unsigned beyond OBS's own repository signing.
- Pre-releases get no OBS builds (see above).
- No AppStream metadata is installed, so software centers show no
  description or screenshots.
- Packages do not ship xremap; first-run setup downloads it (see
  [Upcoming Features](../docs/Upcoming_Features.md)).

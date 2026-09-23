# Setup wizard test matrix

Every state first-run setup can show, with an identifier to name a test
after. The wizard (`src/ui/overlays.rs`, `src/setup.rs`) renders each
page from one value, `setup::Facts`, plus a few fields of the open
wizard (`app::Setup`), so a test stages a state by building those
directly. Nothing is read from or written to the machine the tests
run on: no group is joined, no file lands in `/etc`, no `systemctl`
runs, and CI needs nothing beyond the Rust toolchain.

Three kinds of test use this list:

- **Interface tests** (`src/app/e2e.rs`): stage a state, build the
  window the way the runtime composes it, and drive it with `iced_test`:
  widgets are found by the text they show and clicked, and the messages
  they emit go through `update` as at runtime. Every row of a step's
  table is checked for its status, title, and main button, for what its
  details say, and for what pressing the button does; the storyboards
  are driven by clicks with an assertion at every frame. These run in
  the ordinary `cargo test`, so a regression fails the build. They
  render with iced's software backend, chosen by `ICED_TEST_BACKEND` in
  `.cargo/config.toml`. A failure's message lists every text on the
  window; the failing test also leaves a picture of the window as it
  was under `target/setup-shots/failures/`, which CI uploads with the
  failed run. Prose with a link in it is rich text, which cannot be
  found by its content, so a note that carries a link is only pictured.
- **Screenshots** (`src/app/screenshots.rs`): the same rows and flows,
  rendered with the software renderer and written as PNGs to
  `target/setup-shots/<section>/<ID>-<slug>.png` for review (storyboards
  under `storyboard/<ID>/<frame>-<slug>.png`). Never a pixel assertion;
  a person, or CI's artifact upload, looks at them for what an assertion
  cannot judge: wrapping, contrast, the look of a page. Ignored by
  default: `cargo test --locked screenshots -- --ignored`, or one
  section such as `screenshots::service`. Each section keeps one
  renderer for its captures, which take a few seconds each.
- **Behavior** (`src/setup.rs`, `src/app.rs` tests): what a fix does to
  files and the service, against a temporary directory and a recorded
  stand-in for `systemctl`, and what the wizard records when it closes.

Names in *italics* are the status and title the page shows; **bold**
is its main button. "Continue" means the page has nothing to fix.

## Pages around the steps

| ID | State | Shows |
| --- | --- | --- |
| W1 | Welcome, checks still running (`facts: None`, `probing`) | *First-run setup* · **Start setup** disabled or waiting |
| W2 | Welcome, checks done | **Start setup**, "Set up later" |
| F1 | Finish, every step in order (`is_all_ok`) | remapping works now |
| F2 | Finish, only a login outstanding (`is_configured`) | works after a restart |
| F3 | Finish, a step left that Keyloom can act on | the step listed with its status, resumes there (`resume_step`) |
| F4 | Finish, steps left that Keyloom cannot act on (no systemd, no home) | listed, closes only |
| F5 | Reopened from the menu after completion | every step Continue, then F1 (captured as storyboard SB5) |
| F6 | Finish, checks still running (`facts: None`) | *Checking your system…* |

## Step 1 · xremap

`XremapCheck`, `Facts::xremap_action`, `install::asset`, `AppMatching`.

| ID | State | Shows |
| --- | --- | --- |
| X1 | Found, the user's own build, any version (`managed: false`) | *Installed · xremap is installed* · Continue |
| X2 | Found, Keyloom's download, current release (`managed: true`, `version == install::RELEASE`) | *Installed · xremap is installed* · Continue |
| X3 | Found, Keyloom's download, older release | *Update available · Update xremap* · **Update xremap** |
| X4 | Found, `--version` unreadable (`version: None`) | *Installed*, version not shown |
| X5 | Missing, a release for this processor and a home to put it in | *Not installed · Install xremap* · **Install xremap** |
| X6 | Missing, no release for this processor or no home (`xremap_action: None`) | *Not installed · Install xremap* · **Check again**, link to xremap's installation guide. Decided by the build's architecture (`install::asset`), not by `Facts`: on a build without a download, the tables stage X6 in place of X5, and X3 shows X1's page, so a build for another processor covers it |

Application-matching note on the page (`app_matching`), on top of any
row above that found a binary:

| ID | State | Shows |
| --- | --- | --- |
| XA1 | Build lists this desktop (`Supported`, e.g. COSMIC) | no note; details say app-specific remaps work (the same image as X1) |
| XA2 | `Supported(Gnome)` on Wayland | note linking xremap's GNOME Shell extension |
| XA3 | `Supported(Gnome)` on X11 (`session.x11`) | no extension note |
| XA4 | Build lists other desktops only (`Unsupported`) | note that app-specific remaps will not work here, naming what it supports |
| XA5 | Build older than 0.15.13 (`desktops: None`, `Unreported`) | details: xremap picks on its own |
| XA6 | Desktop not recognized (`session.desktop: None`, `UnknownDesktop`) | details: xremap picks on its own |

## Step 2 · keyboard access (`input` group)

`GroupCheck`, `Facts::user`.

| ID | State | Shows |
| --- | --- | --- |
| G1 | `Effective` | *Allowed · Keyboard access is allowed* · Continue |
| G2 | `NeedsLogin` (on record, not in this session) | *Takes effect after a restart* · Continue |
| G3 | `NotMember`, user name known | *Not allowed yet · Allow keyboard access* · **Allow keyboard access** (asks for the password) |
| G4 | `NotMember`, user name unknown (`user: None`) | same status and title · **Check again**, body says to run the command under details (`$USER`) |
| G5 | `NoGroup` (no `input` group on this system) | *No input group · Keyboard access isn't available* · Continue |

## Step 3 · virtual keyboard (`/dev/uinput`)

`UinputCheck`, together with the group step.

| ID | State | Shows |
| --- | --- | --- |
| U1 | `Writable` | *Allowed · Virtual keyboard is allowed* · Continue |
| U2 | `Missing`, no rule installed (module not loaded) | *Not set up · Allow the virtual keyboard* · **Allow virtual keyboard** |
| U3 | `Missing`, rule installed | same as U2 |
| U4 | `NotWritable`, no rule installed | *Not allowed yet · Allow the virtual keyboard* · **Allow virtual keyboard** |
| U5 | `NotWritable`, rule installed, group `NeedsLogin` | *Takes effect after a restart* · Continue |
| U6 | `NotWritable`, rule installed, group `Effective` (the rule is not working) | *Not in effect · Allow the virtual keyboard* · **Set up again** |

## Step 4 · remapping service

`UnitCheck`, with access (`has_effective_access`) and whether a unit
can be written (`installable`: a binary and a config path).

| ID | State | Shows |
| --- | --- | --- |
| S1 | Keyloom's unit, running and enabled | *Running · Remapping is on* · Continue |
| S2 | Keyloom's unit, enabled, not running, no effective access, login pending | *Starts after a restart · Remapping is set up* · Continue |
| S3 | Keyloom's unit, enabled, not running, no effective access, nothing pending | same title, status without the login · Continue |
| S4 | Keyloom's unit, enabled, not running, access effective | *Not running · Turn remapping back on* · **Start remapping** |
| S5 | Keyloom's unit, running, not enabled | *Doesn't start at login · Keep remapping on* · **Start at login** |
| S6 | Keyloom's unit, not running, not enabled (also the state right after saving the file by hand) | *Turned off · Turn on remapping* · **Turn on remapping**; details show `systemctl --user enable --now` |
| S7 | `Stale`, running (ours, but the binary, config, or desktop moved) | *Needs an update · Update remapping* · **Update remapping** |
| S8 | `Stale`, not running | same as S7 |
| S9 | `Foreign`, its command loads `keyloom.yml`, running | *Running (your own service) · Your xremap service works with Keyloom* · Continue |
| S10 | `Foreign`, loads `keyloom.yml`, not running | *Not running (your own service) · Start your xremap service* · **Start service** |
| S11 | `Foreign`, its command does not load `keyloom.yml`, running | *Doesn't use Keyloom's remaps · Replace your xremap service?* · **Replace service** and "Keep mine"; `ExecStart` shown |
| S12 | `Foreign`, does not load it, not running | same as S11 |
| S13 | `Foreign`, unit file unreadable (`exec_start` empty) | same as S11, no command to show |
| S14 | `Missing`, installable | *Not set up · Turn on remapping* · **Turn on remapping**; details offer saving the file to turn on by hand |
| S15 | Any state needing the unit written, xremap missing | *Needs xremap · Turn on remapping* · **Back to xremap** |
| S16 | Any state needing the unit written, no config path (no home) | *No home folder · Remapping can't be turned on here* · Continue |
| S17 | `Unavailable` (no systemd user session), binary and config known | *No systemd user session · Remapping can't be turned on here* · Continue; details show the command to run xremap by hand |
| S18 | `Unavailable`, without a binary or config | same, details without the command |

## Transient states, on any step

Fields of `app::Setup`; each combines with the rows above. All are
asserted on: T1 and T6 on every row with a fix or details, the others
in tests or storyboards of their own.

| ID | State | Shows |
| --- | --- | --- |
| T1 | `busy: Some(step)` — the fix is running | main button shows its working label (*Installing…*, *Updating…*, *Waiting for approval…*, *Turning on…*, *Replacing…*, *Starting…*), controls held |
| T2 | `error: Cancelled` — the authentication prompt was dismissed | "Couldn't finish this step: authorization was cancelled…" above the button, fix still offered |
| T3 | `error: NotAuthorized` | "…authorization was refused…", fix still offered |
| T4 | `error: NoPolkit` — no `pkexec` | the fix becomes **Check again**; details open on their own with the commands to run |
| T5 | `error: Failed(text)` — the fix ran and failed (download, `systemctl`, `usermod`) | the tool's own message, fix still offered |
| T6 | `details: true` | "Hide details" and the panel: what changes, how to do it by hand, a Copy button where there are commands |
| T7 | `details` opened by a failure (`details_for_failure`) | as T6; closes again once the step moves on (captured as T4) |
| T8 | `copied: true` | the Copy button acknowledging |
| T9 | `saving: true` (service step) | the save-file control working |

## Cross-cutting

| ID | Variation | Why it matters |
| --- | --- | --- |
| C1 | X11 session (`session.x11`) | the unit skips waiting for a Wayland socket; captured as XA3 |
| C2 | Light theme | contrast of status colors and the modal backdrop; screenshots only |
| C3 | Window at its minimum size | pages taller than the window scroll; footer buttons must stay reachable, which the interface tests press at this size |
| C4 | Long paths and names (`/home/Jo Doe/…`, a long `ExecStart`) | wrapping and quoting in details; the whole command must be shown |
| C5 | Details panel with and without commands | Copy only where there is something to copy; checked on every row, captured as T6a–T6c |

## Behavior tests (no screenshots)

What a fix does, against a temporary directory and a recorded `systemctl`.

| ID | Scenario | Expected |
| --- | --- | --- |
| B1 | **Replace service** on S11 | the foreign file copied to `xremap.service.bak`, Keyloom's unit written, `daemon-reload`, `enable`, then `restart` (only with effective access) |
| B2 | **Update remapping** on S7 | Keyloom's unit rewritten in place, no backup, same commands |
| B3 | "Keep mine" on S11, then "Set up later" | nothing written, no command run, setup recorded as Deferred |
| B4 | **Start service** on S10 | `restart` only; their file untouched |
| B5 | Save the file by hand on S14 | file written and `daemon-reload`, unit neither enabled nor started; the page becomes S6 |
| B6 | Classifying a unit: only `ExecStart` decides `reads_config` | a comment, a commented-out `ExecStart`, or an `ExecStartPre` naming the config does not count (covered) |
| B7 | Closing setup from each of F1–F4 | Complete for F1 and F2, Deferred otherwise, never reopening on its own |

## Storyboards

Flows driven by clicks (`storyboards` in `src/app/e2e.rs`), with an
assertion at every frame, so the order of pages is the real state
machine's; the screenshot tests record the same flows, one image per
frame, for review as a strip or a short video. A fix's outcome and the
checks that follow it are supplied (`SetupActed` with `Ok` or an
error, then `SetupProbed` with the facts afterwards); the fix itself
does not run.

| ID | Flow |
| --- | --- |
| SB1 | Fresh system, opened from the ⋯ menu: W1 → W2 → X5 → T1 → X2 → G3 → T1 → G2 → U2 → T1 → U5 → S2 → F2 → closed → the main window's status chip |
| SB2 | Foreign unit kept: W2 → (X1, G1, U1 passed over) → S11 → "Keep mine" → F3 → closed → the main window's status chip |
| SB3 | Authentication refused then granted: G3 → T2 → G3 → T1 → F2 → G2 from the summary |
| SB4 | No polkit: G3 → T4 → (details) → "Check again" → G2 |
| SB5 | Reopened after completion: W2 → F1 → each step from the summary, Continue → F1 → closed (F5) |
| SB6 | Keyloom's download out of date: W2 → X3 → T1 → F1 |

## Not covered by any kind

The checks themselves (`setup::probe`) read `/proc`, `/etc/group`,
udev's rules directories, `/dev/uinput`, and ask `systemctl` and the
xremap binary; they are covered by unit tests over captured text
(`group_check`, `classify_unit`, `unit_check`, `rules_installed`), not
by staging a machine. Running a real `pkexec`, `usermod`, udev, or
xremap stays a manual check on a disposable system (`scripts/qa-reset.sh`).

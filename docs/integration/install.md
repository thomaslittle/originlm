# Install OriginLM

OriginLM has two equal entry points: a control center to see your AI, and SDKs
for apps built on the AI a user already has. Neither requires an OriginLM account.
The packages are published at `0.1.0-alpha.20`, including the Rust facade and its 13-crate closure:

```sh
npm install originlm
pip install originlm==0.1.0a20
cargo add originlm@=0.1.0-alpha.20
```

There are no signed app installers or hosted one-line downloads yet. The source
build below is for development and offline use; compilation is not a 60-second
install.

## See your AI

From a checkout with the [build prerequisites](../../CONTRIBUTING.md) installed:

```sh
npm ci --ignore-scripts
npm run build
npm run dev -- --example --port 5176
```

Open `http://127.0.0.1:5176`. Example data is labeled and does not inspect accounts.
For the GPUI app, including its in-app allowance mini-view:

```sh
cargo run --locked -p originlm-desktop -- --example
```

The real web flow uses `npm run dev -- --port 5176`, starts the local helper and
pairs through the UI. Provider operations remain subject to grants. A native
control center connects using an explicitly issued administrator descriptor:

```sh
cargo run --locked -p originlm-desktop -- --token-file /absolute/path/to/control-admin-token.json
```

Do not use an administrator descriptor in an ordinary consumer app. See
[shared broker grants](shared-broker.md). The desktop build has **no OS tray**;
Ctrl+Shift+M opens a mini-view inside its window. See the [dashboard guide](../users/dashboard.md).

## Build with it

Use [the SDK and framework quickstarts](quickstarts.md) with the published
packages, or with local packed artifacts for source development. Rust can use
`originlm = "=0.1.0-alpha.20"` or a path dependency; Node/Electron and Python can
auto-start the packaged helper. Explicit local helper paths are also supported.

The SDK does not install the dashboard or a background service. A granted,
compatible shared broker is preferred; otherwise the app owns a private helper.
Rust can fall back to an embedded runtime. Authorization denial remains an error.
Protocol mismatch can select a private copy; startup can still fail if that copy
is missing, invalid or cannot launch.

## Optional background service

[Service scripts](service.md) install an already prepared helper archive and
register a user service. They do not install the GPUI dashboard. Hosted releases
and signing remain release gates. Do not paste a placeholder URL into a shell.

## Remove it

Stop source dev servers and close the app. Remove local SDK dependencies using your
package manager. If you explicitly installed the service, run the matching local
uninstaller described in [service.md](service.md). Ordinary uninstall keeps user
data; purge is a separate explicit choice. No OS tray has been installed by the app.

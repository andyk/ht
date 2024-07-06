# ht - headless terminal

`ht` (short for *headless terminal*) is a command line program that wraps an arbitrary other binary (e.g. `bash`, `vim`, etc.) with a VT100 style terminal interface--i.e. a pseudoterminal client (PTY) plus terminal server--and allows easy programmatic access to the input and output of that terminal (via JSON over STDIN/STDOUT). `ht` is built in rust and works on MacOS and Linux.

<img src="https://andykonwinski.com/assets/img/headless-terminal.png" alt="screenshot of raw terminal output vs ht output" align="right" style="width:450px">


## Use Cases & Motivation

`ht` is useful for programmatically interacting with terminals, which is important for programs that depend heavily on the Terminal as UI. It is useful for testing and for getting AI agents to interact with terminals the way humans do.

The original motiving use case was making terminals easy for LLMs to use. I was trying to use LLM agents for coding, and needed something like a **headless browser** but for terminals.

Terminals are one of the oldest and most prolific UI frameworks in all of computing. And they are stateful so, for example, when you use an editor in your terminal, the terminal has to manage state about the cursor location. Without ht, an agent struggles to manage this state directly; with ht, an agent can just observe the terminal like a human does.

## Installing
Download and use [the latest binary](https://github.com/andyk/ht/releases/latest) for your architecture.

## Building

Building from source requires the [Rust](https://www.rust-lang.org/) compiler
(1.74 or later), and the [Cargo package
manager](https://doc.rust-lang.org/cargo/). If they are not available via your
system package manager then use [rustup](https://rustup.rs/).

To download the source code, build the binary, and install it in
`$HOME/.cargo/bin` run:

```sh
cargo install --git https://github.com/andyk/ht
```

Then, ensure `$HOME/.cargo/bin` is in your shell's `$PATH`.

Alternatively, you can manually download the source code and build the binary
with:

```sh
git clone https://github.com/andyk/ht
cd ht
cargo build --release
```

This produces the binary in _release mode_ (`--release`) at
`target/release/ht`. There are no other build artifacts so you can just
copy the binary to a directory in your `$PATH`.

## Usage

Run `ht` to start interactive bash shell running in a PTY (pseudo-terminal).

To launch a different program (a different shell, another program) run `ht
<command> <args...>`. For example:

- `ht fish` - starts fish shell
- `ht nano` - starts nano editor
- `ht nano /etc/fstab` - starts nano editor with /etc/fstab opened

Another way to run a specific program, e.g. `nano`, is to launch `ht` without a
command, i.e. use bash by default, and start nano from bash by sending `nano\r`
("nano" followed by "return" control character) to the process input. See [input
command](#input) below.

Default size of the virtual terminal window is 120x40 (cols by rows), which can
be changed with `--size` argument. For example: `ht --size 80x24`. The window
size can also be dynamically changed - see [resize command](#resize) below.

Run `ht -h` or `ht --help` to see all available options.

## Live terminal preview

ht comes with a built-in HTTP server which provides a handy live terminal preview page.

To enable it, start ht with `-l` / `--listen` option. This will print the URL of
the live preview.

By default it listens on `127.0.0.1` and a system assigned, dynamic port. If you
need it to bind to another interface, or a specific port, pass the address to
the `-l` option, e.g. `-l 0.0.0.0:9999`.

## API

ht provides 2 types of API: STDIO and WebSocket.

The STDIO API allows control and introspection of the terminal using STDIN,
STDOUT and STDERR.

WebSocket API provides several endpoints for getting terminal updates in
real-time. Websocket API is _not_ enabled by default, and requires starting the
built-in HTTP server with `-l` / `--listen` option.

### STDIO API

ht uses simple JSON-based protocol for sending commands to its STDIN. Each
command must be sent on a separate line and be a JSON object having `"type"`
field set to one of the supported commands (below).

Some of the commands trigger [events](#events). ht may also internally trigger
various events on its own. To subscribe to desired events use `--subscribe
[<event-name>,<event-name>,...]` option when starting ht. This will print the
events as they occur to ht's STDOUT, as JSON-encoded objects. For example, to
subscribe to view snapshots (triggered by sending `takeSnapshot` command) use
`--subscribe snapshot` option. See [events](#events) below for a list of
available event types and their payloads.

Diagnostic messages (notices, errors) are printed to STDERR.

#### sendKeys

`sendKeys` command allows sending keys to a process running in the virtual
terminal as if the keys were pressed on a keyboard.

```json
{ "type": "sendKeys", "keys": ["nano", "Enter"] }
{ "type": "sendKeys", "keys": ["hello", "Enter", "world"] }
{ "type": "sendKeys", "keys": ["^x", "n"] }
```

Each element of the `keys` array can be either a key name or an arbitrary text.
If a key is not matched by any supported key name then the text is sent to the
process as is, i.e. like when using the `input` command.

The key and modifier specifications were inspired by
[tmux](https://github.com/tmux/tmux/wiki/Modifier-Keys).

The following key specifications are currently supported:

- `Enter`
- `Space`
- `Escape` or `^[` or `C-[`
- `Tab`
- `Left` - left arrow key
- `Right` - right arrow key
- `Up` - up arrow key
- `Down` - down arrow key
- `Home`
- `End`
- `PageUp`
- `PageDown`
- `F1` to `F12`

Modifier keys are supported by prepending a key with one of the prefixes:

- `^` - control - e.g. `^c` means <kbd>Ctrl</kbd> + <kbd>C</kbd>
- `C-` - control - e.g. `C-c` means <kbd>Ctrl</kbd> + <kbd>C</kbd>
- `S-` - shift - e.g. `S-F6` means <kbd>Shift</kbd> + <kbd>F6</kbd>
- `A-` - alt/option - e.g. `A-Home` means <kbd>Alt</kbd> + <kbd>Home</kbd>

Modifiers can be combined (for arrow keys only at the moment), so combinations
such as `S-A-Up` or `C-S-Left` are possible.

`C-` control modifier notation can be used with ASCII letters (both lower and
upper case are supported) and most special key names. The caret control notation
(`^`) may only be used with ASCII letters, not with special keys.

Shift modifier can be used with special key names only, such as `Left`, `PageUp`
etc. For text characters, instead of specifying e.g. `S-a` just use upper case
`A`.

Alt modifier can be used with any Unicode character and most special key names.

This command doesn't trigger any event.

#### input

`input` command allows sending arbitrary raw input to a process running in the
virtual terminal.

```json
{ "type": "input", "payload": "ls\r" }
```

In most cases it's easier and recommended to use the `sendKeys` command instead.

Use the `input` command if you don't want any special input processing, i.e. no
mapping of key names to their respective control sequences.

For example, to send Ctrl-C shortcut you must use `"\u0003"` (0x03) as the
payload:

```json
{ "type": "input", "payload": "\u0003" }
```

This command doesn't trigger any event.

#### takeSnapshot

`takeSnapshot` command allows taking a textual snapshot of the the terminal view.

```json
{ "type": "takeSnapshot" }
```

This command triggers `snapshot` event.

#### resize

`resize` command allows resizing the virtual terminal window dynamically by
specifying new width (`cols`) and height (`rows`).

```json
{ "type": "resize", "cols": 80, "rows": 24 }
```

This command triggers `resize` event.

### WebSocket API

The WebSocket API currently provides 2 endpoints:

#### `/ws/events`

This endpoint allows the client to subscribe to events that happen in ht.

Query param `sub` should be set to a comma-separated list of desired events.
E.g. `/ws/events?sub=init,snapshot`.

Events are delivered as JSON encoded strings, using WebSocket text message type.

See [events](#events) section below for the description of all available events.

#### `/ws/alis`

This endpoint implements JSON flavor of [asciinema live stream
protocol](https://github.com/asciinema/asciinema-player/blob/develop/src/driver/websocket.js),
therefore allows pointing asciinema player directly to ht to get a real-time
terminal preview. This endpoint is used by the live terminal preview page
mentioned above.

### Events

The events emitted to STDOUT and via `/ws/events` WebSocket endpoint are
identical, i.e. they are JSON-encoded objects with the same fields and payloads.

Every event contains 2 top-level fields:

- `type` - type of event,
- `data` - associated data, specific to each event type.

The following event types are currently available:

#### `init`

Same as `snapshot` event (see below) but sent only once, as the first event
after ht's start (when sent to STDOUT) and upon establishing of WebSocket
connection.

#### `output`

Terminal output. Sent when an application (e.g. shell) running under ht prints
something to the terminal.

Event data is an object with the following fields:

- `seq` - a raw sequence of characters written to a terminal, potentially including control sequences (colors, cursor positioning, etc.)

#### `resize`

Terminal resize. Send when the terminal is resized with the `resize` command.

Event data is an object with the following fields:

- `cols` - current terminal width, number of columns
- `rows` - current terminal height, number of rows

#### `snapshot`

Terminal window snapshot. Sent when the terminal snapshot is taken with the
`takeSnapshot` command.

Event data is an object with the following fields:

- `cols` - current terminal width, number of columns
- `rows` - current terminal height, number of rows
- `text` - plain text snapshot as multi-line string, where each line represents a terminal row
- `seq` - a raw sequence of characters, which when printed to a blank terminal puts it in the same state as [ht's virtual terminal](https://github.com/asciinema/avt)

## Testing on command line

ht is aimed at programmatic use given its JSON-based API, however one can play
with it by just launching it in a normal desktop terminal emulator and typing in
JSON-encoded commands from keyboard and observing the output on STDOUT.

[rlwrap](https://github.com/hanslub42/rlwrap) can be used to wrap STDIN in a
readline based editable prompt, which also provides history (up/down arrows).

To use `rlwrap` with `ht`:

```sh
rlwrap ht [ht-args...]
```

## Python and Typescript libs

Here are some experimental versions of a simple Python and Typescript libraries that wrap `ht`: [htlib.py](https://github.com/andyk/headlong/blob/24e9e5f37b79b3a667774eefa3a724b59b059775/packages/env/htlib.py) and a [htlib.ts](https://github.com/andyk/headlong/blob/24e9e5f37b79b3a667774eefa3a724b59b059775/packages/env/htlib.ts).

TODO: either pull those into this repo or fork them into their own `htlib` repo.

## Possible future work

* update the interface to return the view with additional color and style information (text color, background, bold/italic/etc) also in a simple JSON format (so no dealing with color-related escape sequence either), and the frontend could render this using HTML (e.g. with styled pre/span tags, similar to how asciinema-player does it) or with SVG.
* support subscribing to view updates, to avoid needing to poll (see [issue #9](https://github.com/andyk/ht/issues/9))
* native integration with asciinema for recording terminal sessions (see [issue #8](https://github.com/andyk/ht/issues/8))

## Alternatives and related projects
[`expect`](https://core.tcl-lang.org/expect/index) is an old related tool that let's you `spawn` an arbitrary binary and then `send` input to it and specify what output you `expect` it to generate next.

Also, note that if there exists an explicit API to achieve your given task (e.g. a library that comes with the tool you're targeting), it will probably be less bug prone/finicky to use the API directly rather than working witht your tool through `ht`.

See also [this hackernews discussion](https://news.ycombinator.com/item?id=40552257) where a bunch of other tools were discussed!

## Design doc

Here is [the original design doc](https://docs.google.com/document/d/1L1prpWos3gIYTkfCgeZ2hLScypkA73WJ9KxME5NNbNk/edit) we used to drive the project development.

## License

All code is licensed under the Apache License, Version 2.0. See LICENSE file for
details.

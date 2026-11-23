# A shopping list program for SailfishOS

A p2p syncing shopping list program for SailfishOS. Has support for multiple
shopping lists. Lists can be shared with other users of this program, changes
are synchronised using [Iroh](https://www.iroh.computer/) without the need for
cloud services or user accounts[^1].

<div style="display: flex; flex-direction: row; gap: 1em">
    <img src="docs/homescreen.png" width="150" />
    <img src="docs/list_view.png" width="150" />
</div>

### Features

- Separate lists which can individually be shared
- List items can be added, marked and unmarked as completed, renamed,
  rearranged, and deleted.
- List synchronisation is peer-to-peer (Currently only occurs when the app is
  open on both devices!)


### Hacking

- Use `just rpm debug` / `just rpm release` to build and package the program.
- To test the synchronisation system, you can use the debug console `just console`.

### Repo notes

- The devenv of this repo does some magic to pull the SDK libraries out of the
  `whisperfish/sailo-rs` sailfish Rust SDK image. Though only the SDK platform
  libraries are extracted, so this is possibly overkill and I could possibly get
  the SDK libraries from somewhere else?.

- The program is built using `cargo zigbuild`, which means no cross-compile
  container/VM/emulator is needed, we simply use the zig compiler to
  cross-compile and link the libraries. You can build this repo from an
  x86-64-darwin machine if you wanted.


### Thanks

This repo borrows a lot of architecture ideas from
[Whisperfish](https://gitlab.com/whisperfish/whisperfish/) and
[Indra](https://git.agnos.is/projectmoon/indra), particularly the choice to use
the actor model.


[^1]: The program uses Iroh relays to perform discovery and route traffic if no
    direct route can be made.

# Real-data inputs

What this crate's real-data tests read, as tests/inputs/mod.rs declares them: the keys of the configuration file
the `GAME_DATA` environment variable names (its layout is described in
[primase-testdata](https://github.com/Kalmarv/primase-testdata)), the file sets and the files of each, and the debug keys
`GAME_DATA_DEBUG` sets. A test whose input is not configured skips with a line naming its key. A `gate` input fails
instead of skipping when the configuration file sets `[run] require = 'gate'`.
The files are your own copies of game files; none are distributed.

<!-- real-data inputs: made by primase_testdata::doc::tables from tests/inputs/mod.rs -->

#### Configuration keys

| key | gate | about | source |
| --- | --- | --- | --- |
| `scratch.root` | - | where real-data tests write their outputs (the OS temporary folder when unset) | - |

#### Oracle sets (`<oracles.root>/<set>/<game>/`)

| set | about | source |
| --- | --- | --- |
| `oracles.corpus` | the games' own files cut from their installs (here: Havok tagfiles and their compendium) | the user's own installs: files extracted as stored |

#### Declared files

| key | status | gate | about | source |
| --- | --- | --- | --- | --- |
| `oracles.corpus.sekiro/havok` | live | yes | one area's high-resolution map collision as its binder stores it: loose Havok 2016.2 tagfiles (`.hkx`) and the type compendium they share (`.compendium`), every body and compressed mesh read by tests/corpus.rs | the game's own files from the user's install: the area's `h*.hkxbhd`/`.hkxbdt` binder with each member taken out of its DCX envelope, as [corpus_sekiro_havok.rs](https://github.com/Kalmarv/primase/tree/main/worlds/fromsoft/examples/corpus_sekiro_havok.rs) in primase does |

<!-- end of real-data inputs -->

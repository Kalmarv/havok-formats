//! The real-data inputs this crate's tests read, each with what it is, where it comes from and whether a run that
//! requires every gate input fails without it (`primase_testdata::inputs!`; README.md here lists them). Integration tests reach it
//! with `mod inputs;`, unit tests with `#[cfg(test)] #[path = "../tests/inputs/mod.rs"] mod inputs;` in the
//! crate root, and tests/real_data_inputs.rs checks that every query the crate's sources write is declared
//! here. Declare an input here when a test first asks for it.

primase_testdata::inputs! {
    games {
        Sekiro = "sekiro",
        Elden = "elden",
    }
    installs {}
    keys {}
    tools {}
    oracle_sets {
        Corpus = "corpus" {
            about: "the games' own files cut from their installs (here: Havok tagfiles and their compendium)",
            source: "the user's own installs: files extracted as stored",
        },
    }
    declared {
        Query::Oracle(OracleSet::Corpus, Game::Sekiro, "havok") => {
            status: Live,
            gate: true,
            about: r#"one area's high-resolution map collision as its binder stores it: loose Havok 2016.2 tagfiles (`.hkx`) and the type compendium they share (`.compendium`), every body and compressed mesh read by tests/corpus.rs"#,
            source: r#"the game's own files from the user's install: the area's `h*.hkxbhd`/`.hkxbdt` binder with each member taken out of its DCX envelope, as [corpus_sekiro_havok.rs](https://github.com/Kalmarv/primase/tree/main/worlds/fromsoft/examples/corpus_sekiro_havok.rs) in primase does"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Sekiro, "havok-2015") => {
            status: Live,
            gate: false,
            about: r#"the one SDK 2015.1 tagfile of the Sekiro install (`c1090`'s clip `a000_205000`: 54 transform tracks, 36 frames), loose; tests/sdk2015.rs reads it, tests/spline_encode.rs writes its blocks back, re-encodes it and writes it into its file"#,
            source: r#"the game's own file from the user's install: the member of `chr/c1090.anibnd.dcx` out of its DCX envelope, found by reading every Havok tagfile of the install for the SDK string `20150100`"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Sekiro, "havok-float") => {
            status: Live,
            gate: false,
            about: r#"two Sekiro cutscene tagfiles with a float track (the camera's field of view): `s11_00_0020` cut0010 (two 16-bit spline blocks) and `s11_01_0031` cut0000 (a spline block, then a static one), loose, each with its own type table; tests/float_tracks.rs reads them, tests/spline_encode.rs writes their blocks back, re-encodes them and writes them into their files"#,
            source: r#"the game's own files from the user's install: the members `cut0010/a0000.hkx` of `cutscene/s11_00_0020.cutscenebnd.dcx` and `cut0000/a0000.hkx` of `cutscene/s11_01_0031.cutscenebnd.dcx`, as stored, found by reading every cutscene binder of the install for animations with float tracks"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Sekiro, "havok-l") => {
            status: Live,
            gate: false,
            about: r#"the one Sekiro tagfile whose mesh trees have sections of page above 0 (89 sections; the low-resolution map collision `l10_00_00_00`, member `_000001`) and the compendium it shares, loose; tests/paged_mesh.rs decodes every mesh of it"#,
            source: r#"the game's own files from the user's install: the members of the `l10_00_00_00.hkxbhd`/`.hkxbdt` binder out of their DCX envelopes, picked by reading every map `l` binder of the install for sections of page above 0"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Sekiro, "havok-cloth") => {
            status: Live,
            gate: false,
            about: r#"every distinct tagfile with cloth (`hclClothData` in its type table) of the install: 149 files, loose, and `manifest.tsv` (sha256, file, compendium, binder, member) naming each of the 167 binder members they were found in; tests/cloth_corpus.rs reads them all"#,
            source: r#"the game's own files from the user's install: every archive entry walked (named or not, nested BND4 and DCX opened), each TAG0 member whose type table has `hclClothData` taken out of its DCX envelope and deduplicated by sha256 (95 `chrbnd`, 36 `objbnd`, 36 `partsbnd` members)"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Elden, "havok-cloth") => {
            status: Live,
            gate: false,
            about: r#"every distinct tagfile with cloth of the install: 814 files, loose, and `manifest.tsv` naming each of the 1,605 binder members they were found in; tests/cloth_corpus.rs reads them all"#,
            source: r#"the game's own files from the user's install, found as for Sekiro (182 `chrbnd`, 1,418 `partsbnd`, 5 `geombnd` members)"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Elden, "havok-float") => {
            status: Live,
            gate: false,
            about: r#"three Elden Ring cutscene tagfiles, loose, each with its own type table: `s13_00_0040` cut0040 (a camera whose float track is a degree 1 spline) and the two animations of `s12_05_0020` (cut0040, cut0060) whose wing-bone rotation spline stores one control point of four NaNs; tests/float_tracks.rs reads them, tests/spline_encode.rs writes their blocks back, re-encodes them (not the two with NaNs) and writes them into their files"#,
            source: r#"the game's own files from the user's install: the members `cut0040/a0000.hkx` of `cutscene/s13_00_0040.cutscenebnd.dcx` and `cut0040/a0000.hkx` and `cut0060/a0000.hkx` of `cutscene/s12_05_0020.cutscenebnd.dcx`, as stored, found by reading every cutscene binder of the install for float tracks and for rotation floats that are not finite"#,
        },
        Query::Oracle(OracleSet::Corpus, Game::Elden, "havok-l") => {
            status: Live,
            gate: false,
            about: r#"the one Elden Ring tagfile whose mesh trees have sections of page above 0 (6,233 sections, pages 1 to 4; the low-resolution map collision `l14_00_00_00`, member `_000000`) and the compendium it shares, loose; tests/paged_mesh.rs decodes every mesh of it"#,
            source: r#"the game's own files from the user's install: the members of the `l14_00_00_00.hkxbhd`/`.hkxbdt` binder out of their DCX envelopes, picked by reading every map `l` binder of the install for sections of page above 0"#,
        },
    }
    debug {}
}

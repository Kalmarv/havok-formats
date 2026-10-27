//! The real-data inputs this crate's tests read, each with what it is, where it comes from and whether a run that
//! requires every gate input fails without it (`primase_testdata::inputs!`; README.md here lists them). Integration tests reach it
//! with `mod inputs;`, unit tests with `#[cfg(test)] #[path = "../tests/inputs/mod.rs"] mod inputs;` in the
//! crate root, and tests/real_data_inputs.rs checks that every query the crate's sources write is declared
//! here. Declare an input here when a test first asks for it.

primase_testdata::inputs! {
    games {
        Sekiro = "sekiro",
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
    }
    debug {}
}

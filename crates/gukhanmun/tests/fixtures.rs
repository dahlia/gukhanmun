// Gukhanmun: umbrella library that wires the engine and adapters together.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Regression-fixture harness for the umbrella `gukhanmun` crate.
//!
//! This binary is registered with `harness = false`, so the
//! `libtest-mimic` runner reports one test per fixture under
//! `tests/fixtures/<category>/`.  See `tests/common/mod.rs` for the fixture
//! discovery and execution machinery, and `tests/fixtures/SEONBI_NOTICE.md`
//! for the Seonbi porting policy.

mod common;

use libtest_mimic::{Arguments, Failed, Trial, run};

fn main() {
    let args = Arguments::from_args();
    let root = common::fixtures_root();
    let fixtures = common::discover(&root);
    let trials: Vec<Trial> = fixtures
        .into_iter()
        .map(|fixture| {
            let name = fixture.name.clone();
            Trial::test(name, move || match common::run_fixture(&fixture) {
                Ok(()) => Ok(()),
                Err(message) => Err(Failed::from(message)),
            })
        })
        .collect();
    run(&args, trials).exit();
}

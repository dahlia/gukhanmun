// Gukhanmun: Private Python extension with a safely shared conversion runtime.
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

fn main() {
    println!("cargo:rerun-if-env-changed=PYO3_BUILD_EXTENSION_MODULE");
    if cfg!(unix)
        && std::env::var_os("PYO3_BUILD_EXTENSION_MODULE").is_none()
        && let Some(directory) = pyo3_build_config::get().lib_dir()
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{directory}");
    }
}

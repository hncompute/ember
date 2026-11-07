# Upstream source

Copied from crosvm's `serde_keyvalue` and `serde_keyvalue/serde_keyvalue_derive` directories.

- Repository: https://chromium.googlesource.com/crosvm/crosvm/
- Revision: 963ab96b3fd85fa9b5b81c9917224ce017867b33
- License: BSD-3-Clause; original source headers and license text are retained.

The derive source lives in the nested `serde_keyvalue_derive` package,
matching crosvm's directory layout. The parser re-exports `FromKeyValues`
when the `argh_derive` feature is enabled.
Manifests have been adapted to Ember's workspace; parser and macro sources
are copied unchanged.

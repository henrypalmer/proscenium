//! Canonical catalog domain (Milestone 40): a Cinemeta-backed catalog keyed by
//! IMDB id. Later slices add the provider-match index and the stream-resolver
//! registry here. See SPEC.md §19 M40 and docs/spikes/2026-06-29-….

pub mod cinemeta;
pub mod resolver;
pub mod stremio;

//! `solid-identity` — account, pod, WebID, and client-credential traits
//! plus their in-memory reference implementations.
//!
//! This crate is a direct port of the TypeScript identity layer in
//! `@solid/community-server`, covering:
//!
//! | TypeScript source                                       | Rust module             |
//! |---------------------------------------------------------|-------------------------|
//! | `AccountStore` / `GenericAccountStore` / `CookieStore`  | [`account`]             |
//! | `PodStore` / `BasePodStore` / `PodSettings`             | [`pod`]                 |
//! | `WebIdStore` / `BaseWebIdStore`                         | [`webid`]               |
//! | `ClientCredentialsStore` / `BaseClientCredentialsStore` | [`client_credentials`]  |

pub mod account;
pub mod client_credentials;
pub mod pod;
pub mod webid;

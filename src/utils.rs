use std::fmt::Display;

use locks::Mutex;
use smash_arc::{Hash40, SearchLookup};

use crate::{resources::types::FilesystemInfo, search::HashLookup};

static HASH_LOOKUP: Mutex<Option<&'static HashLookup>> = Mutex::new(None);

pub struct PrettyPath {
    lookup: &'static HashLookup,
    components: Vec<Hash40>,
}

impl PrettyPath {
    pub fn sub_range(&self, len: usize) -> Hash40 {
        let mut hash = Hash40(0);
        for component in self.components.iter().take(len).copied() {
            if hash.0 != 0 {
                hash = hash.concat("/");
            }
            hash = hash.concat(component);
        }

        hash
    }

    pub fn components(&self) -> &[Hash40] {
        &self.components
    }

    pub fn replace(&mut self, search: impl Into<Hash40>, replace: impl Into<Hash40>) -> bool {
        let search = search.into();
        let replace = replace.into();

        let mut replaced = false;
        for component in self.components.iter_mut() {
            if *component == search {
                *component = replace;
                replaced = true;
            }
        }

        replaced
    }

    pub fn to_whole(&self) -> Hash40 {
        let mut hash = Hash40(0);
        for component in self.components.iter().copied() {
            if hash.0 != 0 {
                hash = hash.concat("/");
            }
            hash = hash.concat(component);
        }

        hash
    }
}

impl Display for PrettyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for component in self.components.iter() {
            f.write_str("/")?;
            if let Some(pretty) = self.lookup.get(*component) {
                f.write_str(pretty)?;
            } else {
                write!(f, "{:#010x}", component.0)?;
            }
        }

        Ok(())
    }
}

pub trait ConcatHash {
    fn concat(self, extra: impl Into<Hash40>) -> Self;
    fn pretty(self) -> PrettyPath;
}

impl ConcatHash for Hash40 {
    fn concat(self, extra: impl Into<Hash40>) -> Self {
        let raw = self.0;
        let extra_raw = extra.into().0;

        let new = hash40::Hash40(raw).concat(hash40::Hash40(extra_raw)).0;
        Hash40(new)
    }

    fn pretty(self) -> PrettyPath {
        let lookup = hash_lookup();

        let Some(search) = FilesystemInfo::instance().map(|fs| fs.search()) else {
            return PrettyPath {
                lookup,
                components: vec![self],
            };
        };

        let mut components = vec![];

        let mut path = self;
        while let Ok(entry) = search.get_path_list_entry_from_hash(path) {
            components.push(entry.file_name.hash40());
            path = entry.parent.hash40();

            if path == Hash40::from("/") {
                break;
            }
        }

        components.reverse();

        PrettyPath { lookup, components }
    }
}

pub fn string_for_hash(hash: Hash40) -> String {
    hash_lookup()
        .get(hash)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{:#x}", hash.0))
}

/// The shared component-name lookup. It is built once by `init_hash_lookup` and reused by the search folder
/// sort and by every pretty printer, so the table only ever exists once in memory. Falls back to an empty
/// table if something asks for it before initialization.
pub fn hash_lookup() -> &'static HashLookup {
    let mut slot = HASH_LOOKUP.lock();
    if slot.is_none() {
        *slot = Some(Box::leak(Box::new(HashLookup::empty())));
    }
    (*slot).unwrap()
}

pub fn init_hash_lookup(empty: bool) {
    let lookup = if empty {
        HashLookup::empty()
    } else {
        crate::search::get_search_lookup()
    };
    log::info!("Hash lookup initialized with {} component names", lookup.len());
    *HASH_LOOKUP.lock() = Some(Box::leak(Box::new(lookup)));
}

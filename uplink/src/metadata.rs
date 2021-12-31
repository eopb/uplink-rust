//! Storj DCS metadata types.

use crate::Result;

use std::collections::HashMap;
use std::ptr;
use std::time::Duration;
use std::vec::Vec;

use uplink_sys as ulksys;

/// It's a container for custom information of a specific "item".
/// It's provided by the users as key-value pairs which must only contain valid
/// UTF-8 characters. Keys are unique, so only one value can be associated with
/// it.
///
/// By convention an application that stores metadata should prepend to the keys
/// a prefix, for example an application named "Image Board" might use the
/// "image-board:" prefix and a key could be "image-board:title".
#[derive(Default)]
pub struct Custom {
    /// The key-value pairs.
    // `entries` contains `Box<str>` because its key-value pairs represents the
    // [`uplink_sys::UplinkCustomMetadata`] that despite it hold C strings for
    // the key and value it has a length for each one because they can contain
    // NULL bytes and don't guarantee to end with a NULL byte.
    // See `Self::from_uplink_c` method implementation to see how a `str` has
    // to be allocated due to not being conventional C strings.
    entries: HashMap<Box<str>, Box<str>>,

    /// Cached underlying c-bindings representation of this instance that guards
    /// it's lifetime while it's hold by this field or this instance drops.
    /// It's an option because it's only created when calling
    /// [`Self::to_uplink_c`] and hold it meanwhile this instance isn't mutated.
    inner: Option<UplinkCustomMetadataWrapper>,
}

impl Custom {
    /// Creates a new custom metadata container containing the passed entries.
    pub fn with_entries(entries: &[(&str, &str)]) -> Self {
        let mut map = HashMap::with_capacity(entries.len());

        for e in entries {
            map.insert(e.0.into(), e.1.into());
        }

        Self {
            entries: map,
            inner: None,
        }
    }

    /// Creates a custom metadata instance from type exposed by the uplink
    /// c-bindings.
    ///
    /// The function makes an copy of all the data that `uc_custom` contains, so
    /// the caller can free it without worries about the created `Self`
    /// instance.
    ///
    /// NOTE this method assumes `uc_custom` only contains key-value pairs that
    /// have valid UTF-8 bytes, if it turns they it doesn't then the mapped
    /// key-value may not have the same value in that byte position and it isn't
    /// either guarantee that the same invalid UTF-8 byte produces the same
    /// mapped value.
    // At this time, it never returns an error, however the return type is a
    // `Result` because it the future may return errors and changing the
    // signature when that happen would be a breaking change, while returning a
    // `Result` at this time doesn't have any trade-off.
    pub(crate) fn from_uplink_c(uc_custom: &ulksys::UplinkCustomMetadata) -> Result<Self> {
        if uc_custom.count == 0 {
            return Ok(Default::default());
        }

        let mut entries = HashMap::<Box<str>, Box<str>>::with_capacity(uc_custom.count as usize);
        // SAFETY: we trust that the underlying c-binding contains a valid
        // pointer to entries and the counter has the exact number of entries,
        // and each entry has a key-value C string with exactly the length
        // specified without leaning that they end with the NULL byte because
        // they could contain NULL bytes.
        unsafe {
            use crate::helpers::unchecked_ptr_c_char_and_length_to_str;

            for i in 0..uc_custom.count as isize {
                let entry = uc_custom.entries.offset(i) as *const ulksys::UplinkCustomMetadataEntry;
                let key = unchecked_ptr_c_char_and_length_to_str(
                    (*entry).key,
                    (*entry).key_length as usize,
                );
                let value = unchecked_ptr_c_char_and_length_to_str(
                    (*entry).value,
                    (*entry).value_length as usize,
                );

                entries.insert(key, value);
            }
        }

        Ok(Self {
            entries,
            inner: None,
        })
    }

    /// Returns the current number of entries (i.e. key-value pairs).
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Gets the entry's value associated with the passed key. Returns none if
    /// there isn't any entry associated to the key.
    pub fn get(&self, key: &str) -> Option<&str> {
        match self.entries.get(key) {
            Some(v) => Some(v),
            None => None,
        }
    }

    /// Inserts a new entry with the specified key and value, returning false if
    /// the key didn't exit, otherwise true and replace the value associated to
    /// the key.
    pub fn insert(&mut self, key: &str, value: &str) -> bool {
        self.inner = None;
        self.entries.insert(key.into(), value.into()).is_some()
    }

    /// An iterator for visiting all the metadata key-value pairs.
    pub fn iter(&self) -> impl std::iter::Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }

    /// Deletes the entry with the associated key, returning false if the key
    /// didn't exist, otherwise true.
    pub fn delete(&mut self, key: &str) -> bool {
        self.inner = None;
        self.entries.remove(key).is_some()
    }

    /// Returns the underlying c-bindings representation of this custom metadata
    /// container which is valid as long as `self` isn't mutated or dropped.
    ///
    /// When this method is called more than once and `self` isn't mutated
    /// in between, the calls after the first are very cheap because the
    /// returned value is cached.
    pub(crate) fn to_uplink_c(&mut self) -> ulksys::UplinkCustomMetadata {
        if self.inner.is_none() {
            self.inner = Some(UplinkCustomMetadataWrapper::from_custom(self));
        }

        self.inner.as_ref().unwrap().custom_metadata
    }
}

impl Clone for Custom {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            inner: None,
        }
    }
}

/// It allows to create an [`uplink_sys::UplinkCustomMetadata`] instance that
/// guards the used memory of its list of items during the lifetime of the
/// instance of this struct.
struct UplinkCustomMetadataWrapper {
    /// The [`uplink_sys::UplinkCustomMetadata`] instance that `self`
    /// represents.
    custom_metadata: ulksys::UplinkCustomMetadata,
    /// The allocated memory of the list of entries that `self` guards.
    entries: Vec<ulksys::UplinkCustomMetadataEntry>,
}

impl UplinkCustomMetadataWrapper {
    /// Creates a wrapped [`uplink_sys::UplinkCustomMetadata`]  which represents
    /// the passed [`Custom`].
    fn from_custom(custom: &Custom) -> Self {
        let num_entries = custom.count();
        if num_entries == 0 {
            return Self::default();
        }

        let mut entries = Vec::with_capacity(num_entries);
        for (k, v) in custom.iter() {
            entries.push(ulksys::UplinkCustomMetadataEntry {
                key: k.as_ptr() as *mut i8,
                key_length: k.len() as u64,
                value: v.as_ptr() as *mut i8,
                value_length: v.len() as u64,
            });
        }

        UplinkCustomMetadataWrapper {
            custom_metadata: ulksys::UplinkCustomMetadata {
                entries: entries.as_mut_ptr(),
                count: entries.len() as u64,
            },
            entries,
        }
    }
}

impl Default for UplinkCustomMetadataWrapper {
    fn default() -> Self {
        UplinkCustomMetadataWrapper {
            custom_metadata: ulksys::UplinkCustomMetadata {
                entries: ptr::null_mut(),
                count: 0,
            },
            entries: Vec::new(),
        }
    }
}

/// It's a container of system information of a specific "item".
/// It's provided by the service and only the service can alter it.
pub struct System {
    /// When the associated "item" was created.
    ///
    /// The time is measured with the number of seconds since the Unix Epoch
    /// time.
    pub created: Duration,
    /// When the associated "item" expires. When it never expires is `None`.
    ///
    /// The time is measured with the number of seconds since the Unix Epoch
    /// time.
    pub expires: Option<Duration>,
    /// Then length of the data associated to this metadata.
    ///
    /// NOTE it's a signed integer because the original library uses a signed
    /// integer, so it may be the case now or in the future that negatives
    /// numbers are used.
    pub content_length: i64,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_custom_with_entries() {
        let key1 = "key-a";
        let val1 = "val-a";
        let key2 = "key-b";
        let val2 = "val-b";

        let custom = Custom::with_entries(&[(key1, val1), (key2, val2)]);
        assert_eq!(custom.count(), 2, "count");
        assert_eq!(custom.get(key1), Some(val1), "get: 'key1'");
        assert_eq!(custom.get(key2), Some(val2), "get: 'key2'");
        assert_eq!(custom.get("unexisting"), None, "get 'unexisting'");
    }

    #[test]
    fn test_custom_insert() {
        let key1 = "key-a";
        let val1 = "val-a";
        let val1_2 = "val-a-2";
        let key2 = "key-b";
        let val2 = "val-b";

        let mut custom = Custom::with_entries(&[(key1, val1)]);
        assert_eq!(custom.count(), 1, "count before inserting a new key");
        assert_eq!(custom.get(key2), None, "get 'key2' before inserting it");
        assert!(!custom.insert(key2, val2), "insert 'key2'");
        assert_eq!(custom.count(), 2, "count after inserting a new key");
        assert_eq!(
            custom.get(key2),
            Some(val2),
            "get 'key2' after inserting it"
        );
        assert_eq!(
            custom.get(key1),
            Some(val1),
            "get 'key1' before undating it"
        );
        assert!(custom.insert(key1, val1_2), "insert 'key1' with new value");
        assert_eq!(custom.count(), 2, "count after inserting an existing key");
        assert_eq!(
            custom.get(key1),
            Some(val1_2),
            "get 'key1' after updating it"
        );
    }

    #[test]
    fn test_custom_remove() {
        let key1 = "key-a";
        let val1 = "val-a";
        let key2 = "key-b";
        let val2 = "val-b";

        let mut custom = Custom::with_entries(&[(key1, val1), (key2, val2)]);
        assert_eq!(custom.count(), 2, "count before removing a new key");
        assert!(custom.delete(key1), "remove 'key1'");
        assert_eq!(custom.count(), 1, "count after removing a new key");
        assert_eq!(custom.get(key1), None, "get 'key1'");
        assert_eq!(custom.get(key2), Some(val2), "get 'key2'");
        assert!(!custom.delete(key1), "remove an unexisting key");
        assert_eq!(custom.count(), 1, "count after removing a unexisting key");
    }

    #[test]
    fn test_custom_clone() {
        let mut source = Custom::default();
        assert_eq!(
            source.count(),
            0,
            "count on 'source' after it's initialized with 'default'"
        );

        let clone = source.clone();
        assert_eq!(
            clone.count(),
            0,
            "count on 'clone' after cloning an instance with 0 entries"
        );

        let key1 = "key-a";
        let val1 = "val-a";
        let key2 = "key-b";
        let val2 = "val-b";
        assert!(!source.insert(key1, val1), "insert 'key1' into 'source'");

        let mut clone = source.clone();
        assert_eq!(
            clone.count(),
            1,
            "count on 'clone' after cloning an instance with 1 entries"
        );
        assert_eq!(clone.get(key1), Some(val1), "get 'key1' from 'clone'");

        assert!(!source.insert(key2, val2), "insert 'key2' into 'soure'");
        assert_eq!(
            clone.count(),
            1,
            "count of 'clone' after inserting 'key2' in 'source'"
        );
        assert_eq!(
            clone.get(key1),
            Some(val1),
            "get 'key1' from 'clone' after inserting 'key2' in 'source'"
        );
        assert_eq!(
            clone.get(key2),
            None,
            "get 'key2' from 'clone' which has never been inserted"
        );

        assert!(source.delete(key1), "remove 'key1' from 'soruce'");
        assert_eq!(
            clone.count(),
            1,
            "count on 'clone' after removing 'key1' of 'source'"
        );
        assert_eq!(
            clone.get(key1),
            Some(val1),
            "get 'key1' from 'clone' after remove 'key1' of 'source'"
        );
        assert_eq!(
            source.count(),
            1,
            "count on 'source' before removing 'key1' of 'clone'"
        );
        assert!(clone.delete(key1), "remove 'key1' from 'clone'");
        assert_eq!(
            source.count(),
            1,
            "count on 'source' after removing 'key1' of 'clone'"
        );
    }

    #[test]
    fn test_custom_iterator() {
        let entries = [("key1", "val1"), ("key2", "val2")];

        let custom = Custom::with_entries(&entries);
        assert_eq!(custom.count(), entries.len(), "number of entries");
        for entry in (&custom).iter() {
            if !entries.contains(&entry) {
                panic!("Custom shouln't contains the entry {:#?}", entry);
            }
        }
    }

    use crate::helpers::test::{assert_c_string, compare_c_string};

    #[test]
    fn test_custom_from_uplink_c() {
        let key1 = "key-a";
        let val1 = "val-a";
        let key2 = "key-b";
        let val2 = "val-b";

        let from;
        {
            // This scope drops `to` for doing the commented check right after
            // the scope closes.
            let mut to = Custom::with_entries(&[(key1, val1), (key2, val2)]);
            from =
                Custom::from_uplink_c(&to.to_uplink_c()).expect("to be a valid UplinkCustomMetada");

            assert_eq!(from.count(), 2, "count");
            assert_eq!(from.get(key1), Some(val1), "get: 'key1'");
            assert_eq!(from.get(key2), Some(val2), "get: 'key2'");

            // Ensure that to is dropped.
            drop(to);
        }

        // Check that a Custom instance generated from an UplinkCustomMetadata
        // that has dropped is still valid.
        assert_eq!(from.count(), 2, "count");
        assert_eq!(from.get(key1), Some(val1), "get: 'key1'");
        assert_eq!(from.get(key2), Some(val2), "get: 'key2'");
    }

    #[test]
    fn test_custom_to_uplink_c() {
        let key1 = "key-a";
        let val1 = "val-a";
        let key2 = "key-b";
        let val2 = "val-b";

        let mut custom = Custom::with_entries(&[(key1, val1), (key2, val2)]);

        let c_custom = custom.to_uplink_c();
        assert_eq!(c_custom.count, 2, "count");

        let c_entries = c_custom.entries as *const ulksys::UplinkCustomMetadataEntry;
        unsafe {
            for i in 0..1 {
                let entry = *c_entries.offset(i);

                if compare_c_string(entry.key, key1).is_none() {
                    assert_c_string(entry.value, val1);
                    continue;
                }

                if compare_c_string(entry.key, key2).is_none() {
                    assert_c_string(entry.value, val2);
                    continue;
                }

                panic!("UplinkCustomMetadata instance doesn't contains one of the expected keys ({}, {})", key1, key2);
            }
        }

        // Modify the custom metadata and verify that the methods returns an
        // UplinkCustomMetadata which reflets the current custom metadata state.
        custom.delete(key1);

        let c_custom = custom.to_uplink_c();
        assert_eq!(c_custom.count, 1, "count");

        let c_entries = c_custom.entries as *const ulksys::UplinkCustomMetadataEntry;
        unsafe {
            let entry = *c_entries;
            assert_c_string(entry.key, key2);
            assert_c_string(entry.value, val2);
        }
    }
}
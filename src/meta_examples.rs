//------------ PrefixAs Metadata impl ---------------------------------------

use crate::prefix_record::MergeUpdate;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrefixAs(pub u32);

impl MergeUpdate for PrefixAs {
    type UserDataIn = ();
    type UserDataOut = ();

    fn merge_update(
        &mut self,
        update_record: PrefixAs,
        _: Self::UserDataIn,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0 = update_record.0;
        Ok(())
    }

    fn clone_merge_update(
        &self,
        update_meta: &Self,
        _: &Self::UserDataIn,
    ) -> Result<(Self, Self::UserDataOut), Box<dyn std::error::Error>>
    where
        Self: std::marker::Sized,
    {
        Ok((PrefixAs(update_meta.0), ()))
    }
}

impl std::fmt::Display for PrefixAs {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "AS{}", self.0)
    }
}

// Hash implementation that always returns the same hash, so that all
// records get thrown on one big heap.
// impl std::hash::Hash for PrefixAs {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         0.hash(state);
//     }
// }

/// Tree-wide empty meta-data type
///
/// A special type that indicates that there's no metadata in the tree
/// storing the prefixes. Note that this is different from a tree with
/// optional meta-data.
#[derive(Clone, Copy, Hash)]
pub enum NoMeta {
    Empty,
}

impl std::fmt::Debug for NoMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("")
    }
}

impl std::fmt::Display for NoMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("")
    }
}

impl MergeUpdate for NoMeta {
    type UserDataIn = ();
    type UserDataOut = ();

    fn merge_update(
        &mut self,
        _: NoMeta,
        _: Self::UserDataIn,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn clone_merge_update(
        &self,
        _: &NoMeta,
        _: &Self::UserDataIn,
    ) -> Result<(Self, Self::UserDataOut), Box<dyn std::error::Error>> {
        Ok((NoMeta::Empty, ()))
    }
}
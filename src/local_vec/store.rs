use crate::local_vec::storage_backend::{InMemStorage, StorageBackend};
use crate::local_vec::TreeBitMap;
use crate::store::QueryResult;
use crate::{MatchOptions, PrefixInfoUnit, Stats, Strides};

use routecore::addr::Prefix;
use routecore::record::{MergeUpdate, NoMeta};
use routecore::addr::{IPv4, IPv6};

pub struct Store<Meta: routecore::record::Meta>
where
    Meta: MergeUpdate,
{
    pub v4: TreeBitMap<InMemStorage<IPv4, Meta>>,
    pub v6: TreeBitMap<InMemStorage<IPv6, Meta>>,
}

impl<Meta: routecore::record::Meta + MergeUpdate> Store<Meta> {
    pub fn new(v4_strides: Vec<u8>, v6_strides: Vec<u8>) -> Self {
        Store {
            v4: TreeBitMap::new(v4_strides),
            v6: TreeBitMap::new(v6_strides),
        }
    }
}

impl<'a, Meta: routecore::record::Meta + MergeUpdate> Store<Meta> {
    pub fn match_prefix(
        &'a self,
        search_pfx: &Prefix,
        options: &MatchOptions,
    ) -> QueryResult<'a, Meta> {
        match search_pfx.addr() {
            std::net::IpAddr::V4(addr) => self.v4.match_prefix(
                &PrefixInfoUnit::<IPv4, NoMeta>::new(addr.into(), search_pfx.len()),
                options,
            ),
            std::net::IpAddr::V6(addr) => self.v6.match_prefix(
                &PrefixInfoUnit::<IPv6, NoMeta>::new(addr.into(), search_pfx.len()),
                options,
            ),
        }
    }

    pub fn insert(
        &mut self,
        prefix: &Prefix,
        meta: Meta,
    ) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        match prefix.addr() {
            std::net::IpAddr::V4(addr) => self.v4.insert(PrefixInfoUnit::new_with_meta(
                addr.into(),
                prefix.len(),
                meta,
            )),
            std::net::IpAddr::V6(addr) => self.v6.insert(PrefixInfoUnit::new_with_meta(
                addr.into(),
                prefix.len(),
                meta,
            )),
        }
    }

    // pub fn prefixes(&'a self) -> RecordSet<'a, Meta> {
    //     let rs4 = self
    //         .v4
    //         .store
    //         .prefixes
    //         .iter()
    //         .collect::<RecordSet<'a, Meta>>();
    //     let rs6 = self
    //         .v6
    //         .store
    //         .prefixes
    //         .iter()
    //         .collect::<RecordSet<'a, Meta>>();

    //     RecordSet::<'a, Meta> {
    //         v4: rs4.v4,
    //         v6: rs6.v6,
    //     }
    // }


    pub fn prefixes_iter(&'a self) -> crate::PrefixInfoUnitIter<'a, Meta> {
        let rs4: std::slice::Iter<PrefixInfoUnit<IPv4, Meta>> = self.v4.store.prefixes[..].iter();
        let rs6 = self.v6.store.prefixes[..].iter();

        crate::PrefixInfoUnitIter::<'a, Meta> {
            v4: Some(rs4),
            v6: rs6,
        }
    }

    // pub fn prefixes_iter_as_slice(&'a self) -> std::slice::Iter<PrefixRecord<'a, Meta>> {
    //     self.v4.store.prefixes.as_slice().iter()
    // }

    pub fn prefixes_len(&self) -> usize {
        self.v4.store.prefixes.len() + self.v6.store.prefixes.len()
    }

    pub fn nodes_len(&self) -> usize {
        self.v4.store.get_nodes_len() + self.v6.store.get_nodes_len()
    }

    pub fn nodes_per_stride(&'a self) -> (Vec<u32>, Vec<u32>) {
        let mut stride_levels_v4 = Vec::from([]);
        let mut stride_levels_v6 = Vec::from([]);
        for stride in self.strides().v4.iter().enumerate() {
            stride_levels_v4.push(
                self.v4
                    .stats
                    .iter()
                    .find(|s| s.stride_len == *stride.1)
                    .unwrap()
                    .created_nodes[stride.0]
                    .count as u32,
            );
        }
        for stride in self.strides().v6.iter().enumerate() {
            stride_levels_v6.push(
                self.v6
                    .stats
                    .iter()
                    .find(|s| s.stride_len == *stride.1)
                    .unwrap()
                    .created_nodes[stride.0]
                    .count as u32,
            );
        }
        (stride_levels_v4, stride_levels_v6)
    }

    pub fn stats(&self) -> Stats {
        Stats {
            v4: &self.v4.stats,
            v6: &self.v6.stats,
        }
    }

    pub fn strides(&'a self) -> Strides {
        Strides {
            v4: &self.v4.strides,
            v6: &self.v6.strides,
        }
    }
}

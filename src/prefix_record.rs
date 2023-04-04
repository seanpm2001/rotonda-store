use std::{cmp::Ordering, sync::Arc};
use std::fmt;
use std::fmt::Debug;

use crate::{IPv4, IPv6};
use crate::{af::AddressFamily, local_array::node::PrefixId};
use routecore::addr::Prefix;
use routecore::record::{MergeUpdate, Meta};

//------------ InternalPrefixRecord -----------------------------------------

#[derive(Clone, Copy)]
pub struct InternalPrefixRecord<AF, M>
where
    M: Meta,
    AF: AddressFamily,
{
    pub net: AF,
    pub len: u8,
    pub meta: M,
}

impl<M, AF> InternalPrefixRecord<AF, M>
where
    M: Meta + MergeUpdate,
    AF: AddressFamily,
{
    // pub fn new(net: AF, len: u8) -> InternalPrefixRecord<AF, M> {
    //     Self {
    //         net,
    //         len,
    //         meta: None,
    //     }
    // }
    pub fn new_with_meta(
        net: AF,
        len: u8,
        meta: M,
    ) -> InternalPrefixRecord<AF, M> {
        Self { net, len, meta }
    }

    // This should never fail, since there shouldn't be a invalid prefix in
    // this record in the first place.
    pub fn prefix_into_pub(&self) -> routecore::addr::Prefix {
        routecore::addr::Prefix::new(self.net.into_ipaddr(), self.len)
            .unwrap_or_else(|p| panic!("can't convert {:?} into prefix.", p))
    }

    pub fn get_prefix_id(&self) -> PrefixId<AF> {
        PrefixId::new(self.net, self.len)
    }

    pub fn get_meta(&self) -> &M {
        &self.meta
    }
}

impl<M, AF> std::fmt::Display for InternalPrefixRecord<AF, M>
where
    M: Meta + MergeUpdate,
    AF: AddressFamily,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{} {}",
            AddressFamily::fmt_net(self.net),
            self.len,
            self.meta.summary()
        )
    }
}

impl<AF, M> Ord for InternalPrefixRecord<AF, M>
where
    M: Meta,
    AF: AddressFamily,
{
    fn cmp(&self, other: &Self) -> Ordering {
        (self.net >> (AF::BITS - self.len))
            .cmp(&(other.net >> ((AF::BITS - other.len) % 32)))
    }
}

impl<AF, M> PartialEq for InternalPrefixRecord<AF, M>
where
    M: Meta,
    AF: AddressFamily,
{
    fn eq(&self, other: &Self) -> bool {
        self.net >> (AF::BITS - self.len)
            == other.net >> ((AF::BITS - other.len) % 32)
    }
}

impl<AF, M> PartialOrd for InternalPrefixRecord<AF, M>
where
    M: Meta,
    AF: AddressFamily,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            (self.net >> (AF::BITS - self.len))
                .cmp(&(other.net >> ((AF::BITS - other.len) % 32))),
        )
    }
}

impl<AF, M> Eq for InternalPrefixRecord<AF, M>
where
    M: Meta,
    AF: AddressFamily,
{
}

impl<T, AF> Debug for InternalPrefixRecord<AF, T>
where
    AF: AddressFamily,
    T: Meta,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}/{} with {:?}",
            AddressFamily::fmt_net(self.net),
            self.len,
            self.meta
        ))
    }
}

// impl<AF, T> std::hash::Hash for InternalPrefixRecord<AF, T>
// where
//     AF: AddressFamily + PrimInt + Debug,
//     T: Meta,
// {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.net.hash(state);
//         self.len.hash(state);
//     }
// }

impl<AF, M> From<InternalPrefixRecord<AF, M>> for PrefixId<AF>
where
    AF: AddressFamily,
    M: Meta,
{
    fn from(record: InternalPrefixRecord<AF, M>) -> Self {
        Self::new(record.net, record.len)
    }
}

impl<AF, T> From<&InternalPrefixRecord<AF, T>> for PrefixId<AF>
where
    AF: AddressFamily,
    T: Meta,
{
    fn from(record: &InternalPrefixRecord<AF, T>) -> Self {
        Self::new(record.net, record.len)
    }
}

// impl<'a, AF, M> From<routecore::bgp::PrefixRecord<'a, M>>
//     for InternalPrefixRecord<AF, M>
// where
//     AF: AddressFamily,
//     M: Meta,
// {
//     fn from(record: routecore::bgp::PrefixRecord<'a, M>) -> Self {
//         Self {
//             net: AF::from_ipaddr(record.key().addr()),
//             len: record.key().len(),
//             meta: record.meta().into_owned(),
//         }
//     }
// }

impl<M: Meta> From<PublicPrefixRecord<M>> for InternalPrefixRecord<crate::IPv4, M> {
    fn from(record: PublicPrefixRecord<M>) -> Self {
        Self {
            net: crate::IPv4::from_ipaddr(record.prefix.addr()),
            len: record.prefix.len(),
            meta: record.meta,
        }
    }
}

impl<M: Meta> From<PublicPrefixRecord<M>> for InternalPrefixRecord<crate::IPv6, M> {
    fn from(record: PublicPrefixRecord<M>) -> Self {
        Self {
            net: crate::IPv6::from_ipaddr(record.prefix.addr()),
            len: record.prefix.len(),
            meta: record.meta,
        }
    }
}


//------------ PublicPrefixRecord -------------------------------------------

#[derive(Clone, Debug)]
pub struct PublicPrefixRecord<M: Meta> {
    pub prefix: routecore::addr::Prefix,
    pub meta: M
}

impl<M: Meta> PublicPrefixRecord<M> {
    pub fn new(prefix: Prefix, meta: M) -> Self {
        Self {
            prefix,
            meta 
        }
    }

    pub fn new_from_record<AF: AddressFamily>(record: InternalPrefixRecord<AF, M>) -> Self {
        Self {
            prefix: record.prefix_into_pub(),
            meta: record.meta
        }
    }
}


impl<'a, AF, M> From<Arc<InternalPrefixRecord<AF, M>>>
    for PublicPrefixRecord<M>
where
    AF: AddressFamily,
    M: Meta,
{
    fn from(record: Arc<InternalPrefixRecord<AF, M>>) -> Self {
        Self {
            prefix: record.prefix_into_pub(),
            meta: (*record).meta.clone()
        }
    }
}

impl<M: Meta> std::fmt::Display for PublicPrefixRecord<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} :{:?}", self.prefix, self.meta)
    }
}

impl<'a, M: Meta> From<(Prefix, M)> for PublicPrefixRecord<M> {
    fn from((prefix, meta): (Prefix, M)) -> Self {
        Self {
            prefix,
            meta: meta.clone()
        }
    }
}


//------------ RecordSet ----------------------------------------------------

#[derive(Clone, Debug)]
pub struct RecordSet<M: Meta> {
    pub v4: Vec<PublicPrefixRecord<M>>,
    pub v6: Vec<PublicPrefixRecord<M>>,
}

impl<'a, M: Meta> RecordSet<M> {
    pub fn is_empty(&self) -> bool {
        self.v4.is_empty() && self.v6.is_empty()
    }

    pub fn iter(&self) -> RecordSetIter<M> {
        RecordSetIter {
            v4: if self.v4.is_empty() {
                None
            } else {
                Some(self.v4.iter())
            },
            v6: self.v6.iter(),
        }
    }

    #[must_use]
    pub fn reverse(mut self) -> RecordSet<M> {
        self.v4.reverse();
        self.v6.reverse();
        self
    }

    pub fn len(&self) -> usize {
        self.v4.len() + self.v6.len()
    }
}

impl<'a, M: Meta> fmt::Display for RecordSet<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let arr_str_v4 =
            self.v4.iter().fold("".to_string(), |pfx_arr, pfx| {
                format!("{} {}", pfx_arr, *pfx)
            });
        let arr_str_v6 =
            self.v6.iter().fold("".to_string(), |pfx_arr, pfx| {
                format!("{} {}", pfx_arr, *pfx)
            });

        write!(f, "V4: [{}], V6: [{}]", arr_str_v4, arr_str_v6)
    }
}

impl<'a, M: Meta>
    From<(Vec<PublicPrefixRecord<M>>, Vec<PublicPrefixRecord<M>>)>
    for RecordSet<M>
{
    fn from(
        (v4, v6): (Vec<PublicPrefixRecord<M>>, Vec<PublicPrefixRecord<M>>),
    ) -> Self {
        Self { v4, v6 }
    }
}

impl<'a, M: Meta + 'a>
    std::iter::FromIterator<Arc<PublicPrefixRecord<M>>>
    for RecordSet<M>
{
    fn from_iter<I: IntoIterator<Item = Arc<PublicPrefixRecord<M>>>> (
        iter: I,
    ) -> Self {
        let mut v4 = vec![];
        let mut v6 = vec![];
        for pfx in iter {
            let u_pfx = pfx.prefix;
            match u_pfx.addr() {
                std::net::IpAddr::V4(_) => {
                    v4.push(PublicPrefixRecord::new(u_pfx, pfx.meta.clone()));
                }
                std::net::IpAddr::V6(_) => {
                    v6.push(PublicPrefixRecord::new(u_pfx, pfx.meta.clone()));
                }
            }
        }
        Self { v4, v6 }
    }
}


impl<'a, AF: AddressFamily, M: Meta + 'a>
    std::iter::FromIterator<Arc<InternalPrefixRecord<AF, M>>>
    for RecordSet<M>
{
    fn from_iter<I: IntoIterator<Item = Arc<InternalPrefixRecord<AF, M>>>> (
        iter: I,
    ) -> Self {
        let mut v4 = vec![];
        let mut v6 = vec![];
        for pfx in iter {
            let u_pfx = (*pfx).prefix_into_pub();
            match u_pfx.addr() {
                std::net::IpAddr::V4(_) => {
                    v4.push(PublicPrefixRecord::new(u_pfx, pfx.meta.clone()));
                }
                std::net::IpAddr::V6(_) => {
                    v6.push(PublicPrefixRecord::new(u_pfx, pfx.meta.clone()));
                }
            }
        }
        Self { v4, v6 }
    }
}

impl<'a, AF: AddressFamily, M: Meta + 'a>
    std::iter::FromIterator<&'a InternalPrefixRecord<AF, M>>
    for RecordSet<M>
{
    fn from_iter<I: IntoIterator<Item = &'a InternalPrefixRecord<AF, M>>> (
        iter: I,
    ) -> Self {
        let mut v4 = vec![];
        let mut v6 = vec![];
        for pfx in iter {
            let u_pfx = (*pfx).prefix_into_pub();
            match u_pfx.addr() {
                std::net::IpAddr::V4(_) => {
                    v4.push(PublicPrefixRecord::new(u_pfx, pfx.meta.clone()));
                }
                std::net::IpAddr::V6(_) => {
                    v6.push(PublicPrefixRecord::new(u_pfx, pfx.meta.clone()));
                }
            }
        }
        Self { v4, v6 }
    }
}

impl<'a, M: Meta> std::ops::Index<usize>
    for RecordSet<M>
{
    type Output = PublicPrefixRecord<M>;

    fn index(&self, index: usize) -> &Self::Output {
        if index < self.v4.len() {
            &self.v4[index]
        } else {
            &self.v6[index - self.v4.len()]
        }
    }
}

//------------ RecordSetIter ------------------------------------------------

#[derive(Clone, Debug)]
pub struct RecordSetIter<'a, M: Meta> {
    v4: Option<std::slice::Iter<'a, PublicPrefixRecord<M>>>,
    v6: std::slice::Iter<'a, PublicPrefixRecord<M>>,
}

impl<'a, M: Meta> Iterator for RecordSetIter<'a, M> {
    type Item = PublicPrefixRecord<M>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.v4.is_none() {
            return self.v6.next().map(|res| res.to_owned());
        }

        if let Some(res) = self.v4.as_mut().and_then(|v4| v4.next()) {
            return Some(res.to_owned());
        }
        self.v4 = None;
        self.next()
    }
}
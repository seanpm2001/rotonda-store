use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, AtomicU8};
use std::{
    fmt::Debug,
    marker::PhantomData,
};

use log::{trace, info};
use routecore::record::NoMeta;

pub use super::atomic_stride::*;
pub use crate::local_array::query::*;
pub use crate::local_array::tree::*;
use crate::prefix_record::InternalPrefixRecord;
use crate::af::Zero;
use crate::af::AddressFamily;

//------------ TreeBitMap Node ----------------------------------------------

// The treebitmap turned into a triebitmap, really. A Node in the treebitmap
// now only holds a ptrbitarr bitmap and a pfxbitarr bitmap, that indicate
// whether a node or a prefix exists in that spot. The corresponding node Ids
// and prefix ids are calcaluted from their position in the array. Nodes do
// *NOT* have a clue where they are in the tree, so they don't know the node
// id they represent. Instead, the node id is calculated from the position in
// the tree. That's why several methods take a `base_prefix` as a an argument:
// it represents the ID of the node itself.
//
// The elision of both the collection of children nodes and the prefix nodes in
// a treebitmap node is enabled by the storage backend for the multi-threaded
// store, since holds its entries keyed on the [node|prefix] id. (in contrast
// with arrays or vecs, that have 
pub struct TreeBitMapNode<
    AF,
    S,
> where
    Self: Sized,
    S: Stride,
    AF: AddressFamily,
{
    pub ptrbitarr: <S as Stride>::AtomicPtrSize,
    pub pfxbitarr: <S as Stride>::AtomicPfxSize,
    pub _af: PhantomData<AF>,
}

impl<AF, S> Debug
    for TreeBitMapNode<AF, S>
where
    AF: AddressFamily,
    S: Stride,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeBitMapNode")
            .field("ptrbitarr", &self.ptrbitarr.load())
            .field("pfxbitarr", &self.pfxbitarr.load())
            .finish()
    }
}

impl<AF, S>
    std::fmt::Display for TreeBitMapNode<AF, S>
where
    AF: AddressFamily,
    S: Stride
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TreeBitMapNode {{ ptrbitarr: {:?}, pfxbitarr: {:?} }}",
            self.ptrbitarr.load(),
            self.pfxbitarr.load(),
        )
    }
}

impl<AF, S>
    TreeBitMapNode<AF, S>
where
    AF: AddressFamily,
    S: Stride
{
    // Iterate over all the child node of this node
    pub(crate) fn ptr_iter(&self, base_prefix: StrideNodeId<AF>) -> 
        NodeChildIter<AF,S> {
            NodeChildIter::<AF,S> {
                base_prefix,
                ptrbitarr: self.ptrbitarr.load(),
                bit_span: 0,
                _af: PhantomData,
            }
    }

    // Iteratate over all the prefix ids contained in this node
    pub(crate) fn pfx_iter(&self, base_prefix: StrideNodeId<AF>) -> 
        NodePrefixIter<AF, S> {
        NodePrefixIter::<AF, S> {
            pfxbitarr: self.pfxbitarr.to_u64(),
            base_prefix,
            start_len: 1,
            start_bit_span: 0,
            _af: PhantomData,
            _s: PhantomData,
        }
    }

    // Inspects the stride (nibble, nibble_len) to see it there's already a 
    // child node (if not at the last stride) or a prefix (if it's the last
    // stride).
    //
    // Returns one of:
    // - A newly created child node.
    // - The index of the existing child node in the global `nodes` vec
    // - A newly created Prefix
    // - The index of the existing prefix in the global `prefixes` vec
    pub(crate) fn eval_node_or_prefix_at(
        &mut self,
        nibble: u32,
        nibble_len: u8,
        // all the bits of the search prefix, but with the length set to
        // the length of this stride. So bits are set beyond its length.
        base_prefix: StrideNodeId<AF>,
        stride_len: u8,
        next_stride: Option<&u8>,
        is_last_stride: bool,
    ) -> NewNodeOrIndex<AF> {

        // THE CRIICAL SECTION
        //
        // UPDATING ptrbitarr & pfxbitarr
        //
        // This section is not as critical as creating/updating a
        // a prefix. We need to set one bit only, and if somebody
        // beat us to it that's fine, we'll figure that out when
        // we try to write the prefix's serial number later on.
        // The one thing that can go wrong here is that we are
        // using an old ptrbitarr and overwrite bits set in the
        // meantime elsewhere in the bitarray.
        let ptrbitarr = self.ptrbitarr.load();
        let pfxbitarr = self.pfxbitarr.load();
        let bit_pos = S::get_bit_pos(nibble, nibble_len);
        let new_node: SizedStrideNode<AF>;

        // Check that we're not at the last stride (pfx.len <= stride_end),
        // Note that next_stride may have a value, but we still don't want to
        // continue, because we've exceeded the length of the prefix to
        // be inserted.
        // Also note that a nibble_len < S::BITS (a smaller than full nibble)
        // does indeed indicate the last stride has been reached, but the
        // reverse is *not* true, i.e. a full nibble can also be the last
        // stride. Hence the `is_last_stride` argument
        if !is_last_stride {

            // We are not at the last stride
            // Check it the ptr bit is already set in this position
            if (S::into_stride_size(ptrbitarr) & bit_pos) == <<<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType>::zero() {
                // Nope, set it and create a child node

                match next_stride.unwrap() {
                    3_u8 => {
                        new_node = SizedStrideNode::Stride3(TreeBitMapNode {
                            ptrbitarr: AtomicStride2(AtomicU8::new(0)),
                            pfxbitarr: AtomicStride3(AtomicU16::new(0)),
                            // pfx_vec: PrefixSet::empty(14),
                            _af: PhantomData,
                        });
                    }
                    4_u8 => {
                        new_node = SizedStrideNode::Stride4(TreeBitMapNode {
                            ptrbitarr: AtomicStride3(AtomicU16::new(0)),
                            pfxbitarr: AtomicStride4(AtomicU32::new(0)),
                            // pfx_vec: PrefixSet::empty(30),
                            _af: PhantomData,
                        });
                    }
                    5_u8 => {
                        new_node = SizedStrideNode::Stride5(TreeBitMapNode {
                            ptrbitarr: AtomicStride4(AtomicU32::new(0)),
                            pfxbitarr: AtomicStride5(AtomicU64::new(0)),
                            // pfx_vec: PrefixSet::empty(62),
                            _af: PhantomData,
                        });
                    }
                    _ => {
                        panic!("can't happen");
                    }
                };



                // THE CRIICAL SECTION
                //
                // UPDATING pfxbitarr
                //
                // preventing using an old ptrbitarr and overwrite bits set
                // in the meantime elsewhere in the bitarray.
                let mut a_ptrbitarr = self.ptrbitarr.compare_exchange(ptrbitarr,
                    S::into_ptrbitarr_size(
                    bit_pos | S::into_stride_size(ptrbitarr),
                ));
                loop {
                    match a_ptrbitarr {
                        CasResult(Ok(_)) => {
                            break;
                        }
                        CasResult(Err(newer_array)) => {
                            // Someone beat us to it, so we need to use the
                            // newer array.
                            a_ptrbitarr = self.ptrbitarr.compare_exchange(newer_array,
                                S::into_ptrbitarr_size(
                                bit_pos | S::into_stride_size(newer_array),
                            ));
                        }
                    };
                }

                return NewNodeOrIndex::NewNode(
                    new_node
                );
            }
        } else {
            // only at the last stride do we create the bit in the prefix
            // bitmap, and only if it doesn't exist already
            if pfxbitarr & bit_pos
                == <<<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType as std::ops::BitAnd>::Output::zero()
            {

                // THE CRIICAL SECTION
                //
                // UPDATING pfxbitarr
                //
                // preventing using an old pfxbitarr and overwrite bits set
                // in the meantime elsewhere in the bitarray.
                let mut a_pfxbitarr = 
                self.pfxbitarr.compare_exchange(
                    pfxbitarr, bit_pos | pfxbitarr
                );
                loop {
                    match a_pfxbitarr {
                        CasResult(Ok(_)) => {
                            break;
                        }
                        CasResult(Err(newer_array)) => {
                            // Someone beat us to it, so we need to use the
                            // newer array.
                            a_pfxbitarr = self.pfxbitarr.compare_exchange(
                                newer_array, bit_pos | newer_array
                            );
                        }
                    };
                }

                // self.pfxbitarr.compare_exchange(pfxbitarr, bit_pos | pfxbitarr);
                // CHECK THE RETURN VALUE HERE AND ACT ACCORDINGLY!!!!
                return NewNodeOrIndex::NewPrefix;
            }
            // A prefix exists as a child of the base_prefix, so create the
            // PrefixId with the right offset from the base prefix and cut
            // it off at that point.
            // let pfx: PrefixId<AF> = base_prefix.add_to_len(nibble_len).truncate_to_len().into();
            return NewNodeOrIndex::ExistingPrefix;
        }

        // Nodes always live at the last length of a stride (i.e. the last 
        // nibble), so we add the stride length to the length of the
        // base_prefix (which is always the start length of the stride).
        NewNodeOrIndex::ExistingNode(base_prefix.add_to_len(stride_len).truncate_to_len())
    }

    //-------- Search nibble functions --------------------------------------

    // This function looks for the longest marching prefix in the provided
    // nibble, by iterating over all the bits in it and comparing that with
    // the appriopriate bytes from the requested prefix. It mutates the 
    // `less_specifics_vec` that was passed in to hold all the prefixes found
    // along the way.
    pub(crate) fn search_stride_for_longest_match_at<'a>(
        &'a self,
        search_pfx: &InternalPrefixRecord<AF, NoMeta>,
        mut nibble: u32,
        nibble_len: u8,
        start_bit: u8,
        less_specifics_vec: &mut Option<Vec<PrefixId<AF>>>,
    ) -> (Option<StrideNodeId<AF>>, Option<PrefixId<AF>>) {
        let pfxbitarr = self.pfxbitarr.load();
        let ptrbitarr = self.ptrbitarr.load();
        let mut bit_pos = S::get_bit_pos(nibble, nibble_len);
        let mut found_pfx = None;

        trace!("start longest_match search");
        for n_l in 1..(nibble_len + 1) {
            // Move the bit in the right position.
            nibble =
                AddressFamily::get_nibble(search_pfx.net, start_bit, n_l);
            bit_pos = S::get_bit_pos(nibble, n_l);

            // Check if the prefix has been set, if so select the prefix.
            // This is not necessarily the final prefix that will be
            // returned.

            // Check it there's a prefix matching in this bitmap for this 
            // nibble.
            trace!("pfxbitarr {:032b}", pfxbitarr);

            if pfxbitarr & bit_pos > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero() {
                let f_pfx = PrefixId::new(search_pfx.net.truncate_to_len(start_bit + n_l), start_bit + n_l);
                // f_pfx.set_serial(self.get_pfx_serial(f_pfx, nibble, n_l, guard).load(Ordering::Relaxed));

                // Receiving a less_specifics_vec means that the user wants
                // to have all the last-specific prefixes returned, so add
                // the found prefix.
                trace!("gather pfx in less_specifics {:?}", f_pfx);
                trace!("ls_vec {:?}", less_specifics_vec);
                if let Some(ls_vec) = less_specifics_vec {
                    trace!("len {}", search_pfx.len);
                    trace!("start_bit {}", start_bit);
                    trace!("n_l {}", n_l);
                    trace!("smaller length? {}", search_pfx.len > start_bit + n_l);
                    trace!("{}", (S::into_stride_size(ptrbitarr)
                            & bit_pos)
                            == <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero());
                    if search_pfx.len > start_bit + n_l
                        && (S::into_stride_size(ptrbitarr)
                            & bit_pos)
                            == <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero()
                    {
                        ls_vec.push(f_pfx);
                    }
                }

                found_pfx = Some(f_pfx);
            }
        }

        let base_prefix =
            StrideNodeId::new_with_cleaned_id(search_pfx.net, start_bit);

        // Check if this the last stride, or if they're no more children to
        // go to, if so return what we found up until now.
        if search_pfx.len <= start_bit + nibble_len
            || (S::into_stride_size(ptrbitarr) & bit_pos) == <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero()
        // No children or at the end, return the definitive LMP we found.
        {
            return (
                None,      /* no more children */
                found_pfx, /* The definitive LMP if any */
            );
        }

        // There's another child, return it together with the preliminary LMP
        // we found.
        (
            // The identifier of the node that has children of the next
            // stride.
            Some(base_prefix.add_nibble(nibble, nibble_len)),
            found_pfx,
        )
    }

    // This function looks for the exactly matching prefix in the provided
    // nibble. It doesn't need to iterate over anything it just compares
    // the complete nibble, with the appropriate bits in the requested
    // prefix. Although this is rather efficient, there's no way to collect
    // less-specific prefixes from the search prefix.
    pub(crate) fn search_stride_for_exact_match_at<'a>(
        &'a self,
        search_pfx: &InternalPrefixRecord<AF, NoMeta>,
        nibble: u32,
        nibble_len: u8,
        start_bit: u8,
        _: &mut Option<Vec<PrefixId<AF>>>,
    ) -> (Option<StrideNodeId<AF>>, Option<PrefixId<AF>>) {
        let pfxbitarr = self.pfxbitarr.load();
        let ptrbitarr = self.ptrbitarr.load();
        // This is an exact match, so we're only considering the position of
        // the full nibble.
        let bit_pos = S::get_bit_pos(nibble, nibble_len);
        let mut found_pfx = None;
        let mut found_child = None;

        // Is this the last nibble?
        // Otherwise we're not looking for a prefix (exact matching only
        // lives at last nibble)
        match search_pfx.len <= start_bit + nibble_len {
            // We're at the last nibble.
            true => {
                // Check for an actual prefix at the right position, i.e. 
                // consider the complete nibble.
                if pfxbitarr & bit_pos > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero() {
                    let f_pfx = PrefixId::new(search_pfx.net.truncate_to_len(start_bit + nibble_len), start_bit + nibble_len);
                    found_pfx = Some(f_pfx);
                }
            }
            // We're not at the last nibble.
            false => {
                // Check for a child node at the right position, i.e.
                // consider the complete nibble.
                if (S::into_stride_size(ptrbitarr) & bit_pos) > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero()
                {
                    found_child = Some(
                        StrideNodeId::new_with_cleaned_id(search_pfx.net, start_bit + nibble_len)
                    );
                }
            }
        }

        (
            found_child, /* The node that has children in the next stride, if
                            any */
            found_pfx,   /* The exactly matching prefix, if any */
        )
    }

    // This function looks for the exactly matching prefix in the provided
    // nibble, just like the one above, but this *does* iterate over all the
    // bytes in the nibble to collect the less-specific prefixes of the the 
    // search prefix. This is of course slower, so it should only be used 
    // when the user explicitly requests less-specifics.
    pub(crate) fn search_stride_for_exact_match_with_less_specifics_at(
        &self,
        search_pfx: &InternalPrefixRecord<AF, NoMeta>,
        mut nibble: u32,
        nibble_len: u8,
        start_bit: u8,
        less_specifics_vec: &mut Option<Vec<PrefixId<AF>>>,
    ) -> (Option<StrideNodeId<AF>>, Option<PrefixId<AF>>) {
        let pfxbitarr = self.pfxbitarr.load();
        let ptrbitarr = self.ptrbitarr.load();
        let mut bit_pos = S::get_bit_pos(nibble, nibble_len);
        let mut found_pfx = None;

        let ls_vec = less_specifics_vec
            .as_mut()
            .expect(concat!("You shouldn't call this function without",
            "a `less_specifics_vec` buffer. Supply one when calling this function",
            "or use `search_stride_for_exact_match_at`"));

        for n_l in 1..(nibble_len + 1) {
            // Move the bit in the right position.
            nibble =
                AddressFamily::get_nibble(search_pfx.net, start_bit, n_l);
            bit_pos = S::get_bit_pos(nibble, n_l);

            // Check if the prefix has been set, if so select the prefix.
            // This is not necessarily the final prefix that will be
            // returned.

            // Check it there's a prefix matching in this bitmap for this 
            // nibble.
            if pfxbitarr & bit_pos > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero() {
                // since we want an exact match only, we will fill the prefix
                // field only if we're exactly at the last bit of the nibble
                if n_l == nibble_len {
                    let f_pfx =
                        PrefixId::new(
                            search_pfx.net.truncate_to_len(start_bit + n_l), start_bit + n_l);
                        found_pfx = Some(f_pfx);
                }

                // Receiving a less_specifics_vec means that the user wants to
                // have all the last-specific prefixes returned, so add the
                // found prefix.
                let f_pfx = PrefixId::new(search_pfx.net.truncate_to_len(start_bit + n_l), start_bit + n_l);
                ls_vec.push(f_pfx);
            }
        }

        if found_pfx.is_none() {
            // no prefix here, clear out all of the prefixes we found along
            // the way, since it doesn't make sense to return less-specifics
            // if we don't have a exact match.
            ls_vec.clear();
        }

        // Check if this the last stride, or if they're no more children to 
        // go to, if so return what we found up until now.
        match search_pfx.len <= start_bit + nibble_len
            || (S::into_stride_size(ptrbitarr) & bit_pos)
                == <<<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType as std::ops::BitAnd>::Output::zero()
        {
            // No children or at the end, return the definitive LMP we found.
            true => (
                None,      /* no more children */
                found_pfx, /* The definitive LMP if any */
            ),
            // There's another child, we won't return the found_pfx, since
            // we're not at the last nibble and we want an exact match only.
            false => (
                Some(StrideNodeId::new_with_cleaned_id(search_pfx.net, start_bit + nibble_len)),
                None,
            ),
        }
    }

    // Search a stride for more-specific prefixes and child nodes containing
    // more specifics for `search_prefix`.
    pub(crate) fn add_more_specifics_at(
        &self,
        nibble: u32,
        nibble_len: u8,
        base_prefix: StrideNodeId<AF>,
    ) -> (
        Vec<StrideNodeId<AF>>, /* child nodes with more more-specifics in
                                  this stride */
        Vec<PrefixId<AF>>,     /* more-specific prefixes in this stride */
    ) {
        trace!("start adding more specifics");
        let pfxbitarr = self.pfxbitarr.load();
        let ptrbitarr = self.ptrbitarr.load();
        trace!("ptrbitarr {:032b}", ptrbitarr);
        trace!("pfxbitarr {:032b}", pfxbitarr);
        let mut found_children_with_more_specifics = vec![];
        let mut found_more_specifics_vec: Vec<PrefixId<AF>> = vec![];

        // This is an exact match, so we're only considering the position of
        // the full nibble.
        let mut bit_pos = S::get_bit_pos(nibble, nibble_len);
        let mut found_child = None;

        // Is there also a child node here?
        // Note that even without a child node, there may be more specifics 
        // further up in this pfxbitarr or children in this ptrbitarr.
        if (S::into_stride_size(ptrbitarr) & bit_pos)
            > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero(
            )
        {
            found_child = Some(base_prefix.add_nibble(nibble, nibble_len));
        }

        if let Some(child) = found_child {
            found_children_with_more_specifics.push(child);
        }

        // We're expanding the search for more-specifics bit-by-bit.
        // `ms_nibble_len` is the number of bits including the original
        // nibble we're considering, e.g. if our prefix has a length of 25
        // and we've all strides sized 4, we would end up with a last
        // nibble_len of 1. `ms_nibble_len` will expand then from 2 up and
        // till 4.
        // 
        // ex.:
        // nibble: 1 , (nibble_len: 1)
        // Iteration:
        // ms_nibble_len=1,n_l=0: 10, n_l=1: 11
        // ms_nibble_len=2,n_l=0: 100, n_l=1: 101, n_l=2: 110, n_l=3: 111
        // ms_nibble_len=3,n_l=0: 1000, n_l=1: 1001, n_l=2: 1010, ..., 
        // n_l=7: 1111

        for ms_nibble_len in nibble_len + 1..=S::STRIDE_LEN {
            // iterate over all the possible values for this `ms_nibble_len`,
            // e.g. two bits can have 4 different values.
            for n_l in 0..(1 << (ms_nibble_len - nibble_len)) {
                // move the nibble left with the amount of bits we're going
                // to loop over. e.g. a stride of size 4 with a nibble 0000
                // 0000 0000 0011 becomes 0000 0000 0000 1100, then it will
                // iterate over ...1100,...1101,...1110,...1111
                let ms_nibble =
                    (nibble << (ms_nibble_len - nibble_len)) + n_l as u32;
                bit_pos = S::get_bit_pos(ms_nibble, ms_nibble_len);

                if (S::into_stride_size(ptrbitarr) & bit_pos) > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero()
                {
                    found_children_with_more_specifics.push(
                    base_prefix.add_nibble(ms_nibble, ms_nibble_len)
                    );
                }

                if pfxbitarr & bit_pos > <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero() {
                    found_more_specifics_vec.push(
                        base_prefix.add_nibble(ms_nibble, ms_nibble_len).into()
                        // PrefixId::from(
                        //     base_prefix.get_id().0
                        //     .add_nibble(
                        //         base_prefix.get_id().1, ms_nibble, ms_nibble_len
                        //     ) 
                            // base_prefix.get_id().1 + ms_nibble_len)
                            // .set_serial(self.get_pfx_serial(base_prefix.into(), ms_nibble, ms_nibble_len, guard).load(Ordering::Acquire))
                    )
                }
            }
        }

        trace!("found_children_with_more_specifics {:?}", found_children_with_more_specifics);
        trace!("found_more_specifics_vec {:?}", found_more_specifics_vec);

        (
            // We're done here, the caller should now go over all nodes in
            // found_children_with_more_specifics vec and add ALL prefixes
            // found in there.
            found_children_with_more_specifics,
            found_more_specifics_vec,
        )
    }
}


// ----------- NodeChildIter ------------------------------------------------

// create an iterator over all child nodes id
//
// we don't have a collection of local nodes anymore, since the id of the
// node are deterministically generated, as the prefix+len they represent
// in the treebitmap. This has both the advantage of using less memory,
// and being easier to use in a concurrently updated tree. The
// disadvantage is that we have to look up the child nodes on the fly
// when we want to iterate over all children of a node.
//
// ptr child nodes only exist at the last nibble of the stride size
// (`child_len`). Since children  in the first nibbles are leaf nodes.
// leaf nodes will only be prefixes. So if we have a first stride of
// size 5, all ptr nodes wil have StrideNodeIds with len = 5.
//
// Ex.:
//
// Stride no.          1       2       3      4       5       6       7       
// StrideSize          5       5       4      3       3       3       3      
// child pfxs len      /1-5   /5-10    /10-14 /15-17  /18-20  /21-23  /24-26
// child Nodes len     /5      /10     /14    /17     /20     /23     /26 
//
// Stride no.          8      9
// StrideSize          3      3
// child pfxs len      /27-29 /30-32
// child Nodes len     /29    /32


pub(crate) struct NodeChildIter<AF: AddressFamily, S: Stride> {
   base_prefix: StrideNodeId<AF>,
   ptrbitarr: <<S as Stride>::AtomicPtrSize as AtomicBitmap>::InnerType,
   bit_span: u32, // start with 0
   _af: PhantomData<AF>,
}

impl<'a, AF: AddressFamily, S: Stride> std::iter::Iterator for
    NodeChildIter<AF, S>
{
    type Item = StrideNodeId<AF>;
    fn next(&mut self) -> Option<Self::Item> {
        // iterate over all the possible values for this stride length, e.g.
        // two bits can have 4 different values.
        for cursor in self.bit_span..(1 << S::STRIDE_LEN) {
            // move the bit_span left with the amount of bits we're going to
            // loop over.
            // e.g. a stride of size 4 with a nibble 0000 0000 0000 0011
            // becomes 0000 0000 0000 1100, then it will iterate over 
            // ...1100,...1101,...1110,...1111
            let bit_pos = S::get_bit_pos(cursor, S::STRIDE_LEN);
            if (S::into_stride_size(self.ptrbitarr) & bit_pos) >
                <<S as Stride>::AtomicPfxSize as AtomicBitmap>::InnerType::zero()
            {
                self.bit_span = cursor + 1;
                return Some(self.base_prefix.add_nibble(cursor, S::STRIDE_LEN));
            }    
            
        }
        None
    }
}

// ----------- NodePrefixIter -----------------------------------------------

// Create an iterator of all prefix ids hosted by this node.

// Partition for stride 3
//
// pfxbitarr (AF::BITS)  0 1 2  3  4  5  6   7   8   9  10  11  12  13  14 
// bit_span (binary)     * 0 1 00 01 10 11 000 001 010 011 100 101 110 111 * *
// bit_span (dec.)       * 0 1  0  1  2  3   0   1   2   3   4   5   6   7 * *
// len                   0 1   2           3                               
//
// pfxbitarr (example)   1 0 0 0  0  0  1  1   1   0   0   0   0   0   0   0 0
// pos (example)         0 0 0 0  0  0  0  0   1   0   0   0   0   0   0   0 0                  
//
// Ex.:
// `pos` describes the bit that is currently under consideration. 
// 
// `pfxbitarr` is the bitmap that contains the prefixes. Every 1 in the
// bitmap means that the prefix is hosted by this node. Moreover, the
// position in the bitmap describes the address part of the prefix, given
// a `base prefix`. The descibed prefix is the bits of the `base_prefix`
// bitmap appended by the `bit span` bits.
//
// The length of the prefix is
// described by sum of the length of the base_prefix and the `len`
// variable.
//
// The `bit_span` variable starts counting at every new prefix length.

pub(crate) struct NodePrefixIter<AF: AddressFamily, S: Stride> {
    base_prefix: StrideNodeId<AF>,
    pfxbitarr: u64,
    start_len: u8, // start with 1
    start_bit_span: u32, // start with 0
    _af: PhantomData<AF>,
    _s: PhantomData<S>,
}

impl<'a, AF: AddressFamily, S: Stride> std::iter::Iterator for 
    NodePrefixIter<AF, S> {
        type Item = PrefixId<AF>;

        fn next(&mut self) -> Option<Self::Item> {
            for cur_len in self.start_len..=S::STRIDE_LEN {
                // fancy way of saying the length is muliplied by two every iteration.
                let inc_len = (1 << cur_len) - 1;

                // the bit_span can be a maximum of five bits for a stride of size5
                // (the largest for the multithreaded tree), so that's 0001_1111 and
                // that fits a u8 just fine.
                for bit_span in self.start_bit_span..inc_len + 1 {
                    // shift a 1 all the way to the left, to start counting the
                    // position. 
                    let bit_pos: u64 = (1_u64 << (S::BITS - 1)) >> (inc_len + bit_span);
                    trace!("cmpnibble {:064b} ({} + {})", bit_pos, inc_len, bit_span);
                    trace!("pfxbitarr {:064b}", self.pfxbitarr);
                    if (bit_pos | self.pfxbitarr) == self.pfxbitarr {
                        info!("found prefix with len {} at pos {} pfx len {}", 
                            cur_len, bit_pos, self.base_prefix.get_len());
                        let new_prefix = self.base_prefix
                            .add_nibble(bit_span as u32, cur_len).into();
                        trace!("found prefix {:?}", new_prefix);

                        // the inner for loop gets skipped if self.bit_span
                        // is greater than the `inc_len + 1`, so we can
                        // safely increment it here.
                        self.start_bit_span = bit_span + 1;
                        self.start_len = cur_len;

                        return Some(new_prefix)
                    }
                }
            }
            None
        }     
}
